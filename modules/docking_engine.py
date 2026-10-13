import os
import re
import subprocess
import tempfile
from typing import Optional, Dict
from rdkit import Chem
from rdkit.Chem import AllChem
import meeko

# Définition des cibles thérapeutiques et des centres de poches actives (binding pocket box)
POCKET_CONFIGS = {
    "EGFR_T790M": {
        "pdb": "2JIT",
        "center": (-5.5, 22.5, 26.0),
        "size": (20.0, 20.0, 20.0),
        "description": "Cancer Poumon - Poche ATP du mutant EGFR T790M"
    },
    "SARS_CoV_2_Mpro": {
        "pdb": "7BQ7",
        "center": (-84.8, 15.3, 28.1),
        "size": (22.0, 22.0, 22.0),
        "description": "Covid-19 - Dyade catalytique Cys145/His41"
    },
    "KRAS_G12D": {
        "pdb": "8T5V",
        "center": (20.8, 16.8, -17.2),
        "size": (20.0, 20.0, 20.0),
        "description": "Cancer Pancréas - Poche Switch II du mutant KRAS G12D"
    },
    "BACE1_Alzheimer": {
        "pdb": "1FKN",
        "center": (26.3, 18.1, 20.8),
        "size": (22.0, 22.0, 22.0),
        "description": "Alzheimer - Site actif aspartyl-protéase BACE-1"
    },
    "HER2_Breast": {
        "pdb": "3PP0",
        "center": (25.9, 30.6, 7.6),
        "size": (20.0, 20.0, 20.0),
        "description": "Cancer Sein - Poche kinase récepteur HER2"
    }
}

# Chemin du binaire Vina
VINA_BINARY = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "bin", "vina")


def is_vina_available() -> bool:
    """Vérifie que le binaire AutoDock Vina est présent et exécutable."""
    return os.path.exists(VINA_BINARY) and os.access(VINA_BINARY, os.X_OK)


def smiles_to_pdbqt(smiles: str) -> Optional[str]:
    """
    Génère un conformère 3D optimisé (ETKDGv3 + champ de forces MMFF)
    et le convertit en chaîne PDBQT via Meeko.
    """
    try:
        mol = Chem.MolFromSmiles(smiles)
        if mol is None:
            return None

        # Ajout des hydrogènes indispensables à la 3D et aux interactions polaires
        mol_h = Chem.AddHs(mol)

        # Génération du conformère 3D
        params = AllChem.ETKDGv3()
        params.randomSeed = 42
        cid = AllChem.EmbedMolecule(mol_h, params)
        if cid < 0:
            # Fallback random coordinates si contraintes géométriques serrées
            cid = AllChem.EmbedMolecule(mol_h, useRandomCoords=True)
            if cid < 0:
                return None

        # Minimisation énergétique MMFF94
        try:
            AllChem.MMFFOptimizeMolecule(mol_h, maxIters=200)
        except Exception:
            pass

        # Préparation du ligand pour Vina avec Meeko
        preparator = meeko.MoleculePreparation()
        mol_setups = preparator.prepare(mol_h)
        if not mol_setups:
            return None

        pdbqt_str = meeko.PDBQTWriterLegacy.write_string(mol_setups[0])[0]
        return pdbqt_str
    except Exception:
        return None


def run_docking(smiles: str, target_key: str = "EGFR_T790M", exhaustiveness: int = 1) -> Optional[Dict]:
    """
    Exécute un amarrage moléculaire (docking) structure-based avec AutoDock Vina.
    Retourne un dictionnaire contenant l'énergie libre de liaison (ΔG en kcal/mol)
    et les informations de pose.
    """
    if not is_vina_available():
        return None

    config = POCKET_CONFIGS.get(target_key, POCKET_CONFIGS["EGFR_T790M"])
    pdb_id = config["pdb"]
    base_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    receptor_pdbqt = os.path.join(base_dir, "data", "receptors", f"{pdb_id}.pdbqt")

    if not os.path.exists(receptor_pdbqt):
        return None

    # Conversion du ligand
    ligand_pdbqt = smiles_to_pdbqt(smiles)
    if not ligand_pdbqt:
        return None

    # Fichiers temporaires pour le docking
    with tempfile.NamedTemporaryFile(suffix=".pdbqt", mode="w", delete=False) as f_lig:
        f_lig.write(ligand_pdbqt)
        lig_path = f_lig.name

    out_path = lig_path.replace(".pdbqt", "_out.pdbqt")

    cx, cy, cz = config["center"]
    sx, sy, sz = config["size"]

    cmd = [
        VINA_BINARY,
        "--receptor", receptor_pdbqt,
        "--ligand", lig_path,
        "--out", out_path,
        "--center_x", str(cx),
        "--center_y", str(cy),
        "--center_z", str(cz),
        "--size_x", str(sx),
        "--size_y", str(sy),
        "--size_z", str(sz),
        "--exhaustiveness", str(exhaustiveness),
        "--num_modes", "1",
        "--verbosity", "0"
    ]

    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=15)
        affinity_kcal = None

        # Extraction du score depuis la sortie stdout ou le fichier pdbqt de sortie
        if os.path.exists(out_path):
            with open(out_path, "r") as f_out:
                for line in f_out:
                    if line.startswith("REMARK VINA RESULT:"):
                        parts = line.split()
                        affinity_kcal = float(parts[3])
                        break

        # Fallback analyse stdout
        if affinity_kcal is None and proc.stdout:
            match = re.search(r"^\s*1\s+([-\d\.]+)", proc.stdout, re.MULTILINE)
            if match:
                affinity_kcal = float(match.group(1))

        if affinity_kcal is not None:
            return {
                "target": target_key,
                "pdb": pdb_id,
                "affinity_kcal_mol": round(affinity_kcal, 2),
                "is_favorable": affinity_kcal <= -6.0
            }
        return None
    except Exception:
        return None
    finally:
        for p in [lig_path, out_path]:
            if os.path.exists(p):
                try:
                    os.remove(p)
                except Exception:
                    pass
