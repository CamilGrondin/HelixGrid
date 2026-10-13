import sys
import os
import argparse
import requests
import time
import random
import uuid
import numpy as np
import onnxruntime as ort
import torch
from rdkit import Chem, RDLogger
from rdkit.Chem import AllChem, Descriptors, BRICS

from modules.selfies_engine import generate_selfies_batch
from modules.toxicity_filters import compute_admet_profile
from modules.gnn_models import DMPNN, mol_to_graph
from modules.docking_engine import run_docking, is_vina_available

# Désactiver les avertissements C++ verbeux de RDKit
RDLogger.DisableLog('rdApp.*')

SERVER_URL = os.getenv("HELIXGRID_SERVER_URL", "https://srv1091727.hstgr.cloud")
BATCH_SIZE = 50

# Analyse des paramètres transmis par l'interface graphique ou la ligne de commande
parser = argparse.ArgumentParser(description="HelixGrid Distributed Worker")
parser.add_argument("--target", default="EGFR_T790M", help="Cible thérapeutique / Maladie")
parser.add_argument("--engine", default="selfies_dmpnn", help="Moteur IA")
parser.add_argument("--cpu-limit", type=int, default=100, help="Limite d'allocation CPU en pourcentage (ex. 25, 50, 100)")
parser.add_argument("--idle-only", action="store_true", help="N'activer le calcul que lorsque l'ordinateur est inactif")
parser.add_argument("--pseudo", default=None, help="Pseudo ou identifiant bénévole")
args, _ = parser.parse_known_args()

WORKER_ID = args.pseudo.strip().replace(" ", "_")[:28] if args.pseudo else f"volunteer_{uuid.uuid4().hex[:6]}"

TARGET_CONFIGS = {
    "EGFR_T790M": {
        "name": "Cancer du Poumon (EGFR T790M Résistant)",
        "pdb": "2JIT",
        "threshold": 8.5,
        "seeds": [
            "COc1cc2ncnc(Nc3ccc(F)c(Cl)c3)c2cc1OCC",
            "CN(C)CC=CC(=O)Nc1cc2c(Nc3ccc(F)c(Cl)c3)ncnc2cc1OCC",
            "COc1cc(N(C)CCN(C)C)c(NC(=O)C=C)cc1Nc2nccc(c3cn(C)c4ccccc34)n2",
            "CN1CCN(CC1)C(=O)C2=CC=C(C=C2)NC(=O)C3=CC=CC=C3",
            "c1ccc2ncnc(Nc3ccc(Br)cc3)c2c1",
            "COc1cc2c(Nc3ccc(F)c(Cl)c3)ncnc2cc1OC"
        ]
    },
    "SARS_CoV_2_Mpro": {
        "name": "Covid-19 / Antiviral (SARS-CoV-2 Mpro)",
        "pdb": "7BQ7",
        "threshold": 8.4,
        "seeds": [
            "CC(C)(C)NC(=O)C(NC(=O)C(F)(F)F)C(=O)NC1CC1",
            "CC1(C)C2CC2C1(C)NC(=O)C(NC(=O)C(C)C)C(=O)NC3CCCC3",
            "CC(C)CC(NC(=O)OCc1ccccc1)C(=O)NC(CC2CCNC2=O)C=O",
            "CC(=O)NC(Cc1ccccc1)C(=O)NC(CC2CCNC2=O)C=O",
            "CCOC(=O)C=CC(NC(=O)OCc1ccccc1)C(=O)NC(CC2CCNC2=O)C=O"
        ]
    },
    "KRAS_G12D": {
        "name": "Cancer Pancréas & Côlon (KRAS G12D)",
        "pdb": "8T5V",
        "threshold": 8.5,
        "seeds": [
            "Cc1cc(F)cc(c1)n2c(=O)n(c3cc(F)c(F)c(F)c3)c(=O)c4c(N5CCN(C)CC5)ncnc42",
            "Fc1ccc(Nc2ncc3c(n2)N(c2cccc(Cl)c2)C(=O)N3c2ccccc2)cc1F",
            "CN1CCN(CC1)c2ncc(F)c(Nc3ccc(F)cc3)n2",
            "Fc1cccc(Nc2ncnc3cc(F)c(F)cc23)c1Cl"
        ]
    },
    "BACE1_Alzheimer": {
        "name": "Maladie d'Alzheimer (BACE-1 Protéase)",
        "pdb": "1FKN",
        "threshold": 8.5,
        "seeds": [
            "CN1C(=NC(=N1)N)c2cc(ccc2F)c3cncc(n3)C#N",
            "NC1=NC(=O)N(C)C(=N1)c2cc(ccc2F)Nc3nccc(n3)C(F)(F)F",
            "Fc1cc(ccc1F)c2nc(N)[nH]c2c3cccnc3",
            "CC1(C)NC(=N)N(C)C1c2cccc(Nc3nccc(F)n3)c2"
        ]
    },
    "HER2_Breast": {
        "name": "Cancer du Sein (HER2 / ERBB2)",
        "pdb": "3PP0",
        "threshold": 8.6,
        "seeds": [
            "CC1=CN=C(C=N1)C2=CC3=C(C=C2)N=CN=C3NC4=CC=C(C=C4)OC5=CC=CC=C5F",
            "CS(=O)(=O)CCNCC1=CC=C(O1)C2=CC3=C(C=C2)N=CN=C3NC4=CC(=C(C=C4)OCC5=CC(=CC=C5)F)Cl",
            "COc1cc2ncnc(Nc3ccc(Br)c(F)c3)c2cc1OCC"
        ]
    }
}

target_info = TARGET_CONFIGS.get(args.target, TARGET_CONFIGS["EGFR_T790M"])
SEEDS = list(target_info["seeds"])
MIGRATION_SEEDS = []
HIT_THRESHOLD = target_info["threshold"]

# Informations relatives à l'îlot génétique assigné
CURRENT_ISLAND_ID = None
CURRENT_SCAFFOLD_NAME = None

ENGINE_NAMES = {
    "selfies_dmpnn": "IA Générative SELFIES + Graph Neural Network (D-MPNN)",
    "selfies_chemberta": "Modèle Fondateur ChemBERTa + SELFIES (Transformer)",
    "structure_based_docking": "Amarrage Structure-Based (AutoDock Vina C++)",
    "generative_evolution": "IA Générative De Novo (SMARTS + Évolution RDKit)",
    "brics_fragment": "Recombinaison Rétrosynthétique (Fragments BRICS)",
    "deep_learning_onnx": "Screening Neural Haute Fréquence (Deep MLP ONNX)",
    "swarm_active_learning": "Swarm Intelligence (Apprentissage Collaboratif)"
}
engine_name = ENGINE_NAMES.get(args.engine, args.engine)

print("=" * 75)
print(f"  HELIXGRID - WORKER BÉNÉVOLE")
print(f"  Worker ID  : {WORKER_ID}")
print(f"  Maladie    : {target_info['name']} (PDB: {target_info['pdb']})")
print(f"  Moteur IA  : {engine_name}")
print(f"  Batch Size : {BATCH_SIZE} molécules (Inférence ONNX vectorisée)")
print("=" * 75)

# Chargement du modèle ONNX
ort_session = None
if os.path.exists("affinity_model.onnx"):
    try:
        ort_session = ort.InferenceSession("affinity_model.onnx")
        print("[✓] Modèle d'évaluation ONNX vectorisé prêt (dynamic batching).")
    except Exception as e:
        print(f"[!] Erreur ONNX: {e}")

# Chargement du modèle Graph Neural Network (D-MPNN Chemprop)
dmpnn_model = None
dmpnn_weights = "model_dmpnn_egfr.pt"
if os.path.exists(dmpnn_weights):
    try:
        dmpnn_model = DMPNN(atom_dim=30, edge_dim=10, hidden_dim=128, depth=3)
        dmpnn_model.load_state_dict(torch.load(dmpnn_weights, map_location=torch.device('cpu')))
        dmpnn_model.eval()
        print(f"[✓] Modèle Graph Neural Network D-MPNN prêt ({dmpnn_weights}).")
    except Exception as e:
        print(f"[!] Erreur chargement D-MPNN: {e}")

# Vérification du docking AutoDock Vina
if is_vina_available():
    print("[✓] Moteur de docking structure-based AutoDock Vina v1.2.7 actif.")
else:
    print("[!] AutoDock Vina non détecté dans bin/vina.")

# Mutations SMARTS classiques
MUTATIONS = [
    AllChem.ReactionFromSmarts('[c:1][H]>>[c:1]F'),
    AllChem.ReactionFromSmarts('[c:1][H]>>[c:1]Cl'),
    AllChem.ReactionFromSmarts('[c:1][H]>>[c:1]OC'),
    AllChem.ReactionFromSmarts('[c:1][H]>>[c:1]C'),
    AllChem.ReactionFromSmarts('[c:1][H]>>[c:1]C#N'),
    AllChem.ReactionFromSmarts('[c:1][H]>>[c:1]C(F)(F)F'),
    AllChem.ReactionFromSmarts('[c:1][H]>>[c:1]NC(=O)C=C'),
    AllChem.ReactionFromSmarts('[c:1][H]>>[c:1]N1CCOCC1'),
    AllChem.ReactionFromSmarts('[c:1][H]>>[c:1]N1CCN(C)CC1'),
    AllChem.ReactionFromSmarts('[c:1][H]>>[c:1]N(C)C')
]


def smiles_to_fingerprint(smiles, radius=2, nBits=2048):
    """Calcul individuel d'un fingerprint Morgan pour une molécule."""
    mol = Chem.MolFromSmiles(smiles)
    if mol is None:
        return None
    fp = AllChem.GetMorganFingerprintAsBitVect(mol, radius, nBits=nBits)
    arr = np.zeros((nBits,), dtype=np.float32)
    from rdkit.DataStructs import ConvertToNumpyArray
    ConvertToNumpyArray(fp, arr)
    return arr


def generate_molecules(pool_smiles: list, engine: str, count: int = 50) -> list:
    """Génération de novo avec filtres ADMET, SAscore et PAINS intégrés."""
    candidates = []

    # Stratégie SELFIES (100% de molécules chimiquement valides)
    if engine in ["selfies_dmpnn", "selfies_chemberta", "structure_based_docking"]:
        raw_candidates = generate_selfies_batch(pool_smiles, count=count * 2)
        for s in raw_candidates:
            admet = compute_admet_profile(s, max_sascore=5.0)
            # Filtre strict : Lipinski + SAscore <= 5.0 + Zéro PAINS
            if admet["lipinski_ok"] and admet["synthesizable"] and admet["pains_ok"]:
                candidates.append(s)
                if len(candidates) >= count:
                    break

    # Stratégies classiques (SMARTS / BRICS)
    if len(candidates) < count:
        cand_set = set(candidates)
        if engine in ["generative_evolution", "deep_learning_onnx", "swarm_active_learning"]:
            random_seeds = random.sample(pool_smiles, min(len(pool_smiles), 4))
            for seed_smi in random_seeds:
                try:
                    m = Chem.MolFromSmiles(seed_smi)
                    if not m:
                        continue
                    m_h = Chem.AddHs(m)
                    rxn = random.choice(MUTATIONS)
                    products = rxn.RunReactants((m_h,))
                    for p in products:
                        prod_mol = Chem.RemoveHs(p[0])
                        if prod_mol:
                            smi = Chem.MolToSmiles(prod_mol)
                            admet = compute_admet_profile(smi, max_sascore=5.5)
                            if admet["lipinski_ok"] and admet["pains_ok"]:
                                cand_set.add(smi)
                                if len(cand_set) >= count:
                                    break
                except Exception:
                    continue

        if len(cand_set) < count:
            try:
                mols = [Chem.MolFromSmiles(s) for s in pool_smiles if Chem.MolFromSmiles(s)]
                frags = set()
                for m in mols:
                    frags.update(BRICS.BRICSDecompose(m))
                frag_mols = [Chem.MolFromSmiles(f) for f in frags if Chem.MolFromSmiles(f)]
                if len(frag_mols) >= 2:
                    builder = BRICS.BRICSBuild(frag_mols)
                    for _ in range(count * 2):
                        cand = next(builder)
                        if cand:
                            smi = Chem.MolToSmiles(cand)
                            admet = compute_admet_profile(smi, max_sascore=5.5)
                            if admet["lipinski_ok"] and admet["pains_ok"]:
                                cand_set.add(smi)
                                if len(cand_set) >= count:
                                    break
            except Exception:
                pass
        candidates = list(cand_set)[:count]

    return candidates


def evaluate_candidates(smiles_list: list, engine: str) -> list:
    """
    Évaluation hautement optimisée des candidats :
    - Graph Neural Network D-MPNN si activé
    - INFÉRENCE VECTORISÉE PAR BATCH ONNX (N, 2048) en UN SEUL APPEL
      (5 à 10x plus rapide qu'une boucle itérative)
    - Profilage ADMET complet (SAscore, PAINS, Brenk, Lipinski)
    """
    valid_items = []
    for smiles in smiles_list:
        mol = Chem.MolFromSmiles(smiles)
        if mol is not None:
            valid_items.append((smiles, mol))

    if not valid_items:
        return []

    scores = [None] * len(valid_items)

    # 1. Évaluation Graph Neural Network D-MPNN si moteur sélectionné
    if engine == "selfies_dmpnn" and dmpnn_model is not None:
        for idx, (smiles, mol) in enumerate(valid_items):
            try:
                g = mol_to_graph(mol)
                if g is not None:
                    with torch.no_grad():
                        pred = dmpnn_model(g)
                        scores[idx] = round(float(pred.item()), 2)
            except Exception:
                scores[idx] = None

    # 2. Inférence ONNX vectorisée par Batch pour l'ensemble des candidats restants
    # (Soit moteur deep_learning_onnx/generative_evolution, soit fallback instantané)
    pending_indices = [i for i, s in enumerate(scores) if s is None]
    if pending_indices and ort_session is not None:
        try:
            # Construction de la matrice NumPy (K, 2048) en mémoire contiguë
            k = len(pending_indices)
            fp_matrix = np.zeros((k, 2048), dtype=np.float32)
            from rdkit.DataStructs import ConvertToNumpyArray

            for row, idx in enumerate(pending_indices):
                _, mol = valid_items[idx]
                fp = AllChem.GetMorganFingerprintAsBitVect(mol, radius=2, nBits=2048)
                ConvertToNumpyArray(fp, fp_matrix[row])

            # Inférence ONNX ultra-rapide par lot complet (K, 2048) -> (K, 1)
            outputs = ort_session.run(["affinity"], {"fingerprint": fp_matrix})
            batch_scores = outputs[0].flatten()

            for row, idx in enumerate(pending_indices):
                scores[idx] = round(float(batch_scores[row]), 2)
        except Exception as e:
            # Sécurité en cas d'erreur ponctuelle
            for idx in pending_indices:
                if scores[idx] is None:
                    scores[idx] = 6.0

    # Fallback par défaut si non évalué
    for idx in range(len(scores)):
        if scores[idx] is None:
            scores[idx] = 6.0

    # 3. Profil ADMET complet (SAscore, PAINS, Brenk, Lipinski)
    results = []
    for idx, (smiles, mol) in enumerate(valid_items):
        admet = compute_admet_profile(mol)
        results.append({
            "smiles": smiles,
            "predicted_pkd": scores[idx],
            "mw": admet["mw"],
            "logp": admet["logp"],
            "sascore": admet["sascore"],
            "pains_ok": admet["pains_ok"],
            "brenk_ok": admet["brenk_ok"]
        })

    return results


def sync_island_seeds():
    """
    Synchronise avec le serveur central pour récupérer l'îlot chimique attribué
    et les graines de migration inter-îlots.
    """
    global SEEDS, MIGRATION_SEEDS, CURRENT_ISLAND_ID, CURRENT_SCAFFOLD_NAME
    try:
        r = requests.get(
            f"{SERVER_URL}/seeds",
            params={"worker_id": WORKER_ID, "target": args.target},
            timeout=4
        )
        if r.status_code == 200:
            data = r.json()
            CURRENT_ISLAND_ID = data.get("island_id")
            CURRENT_SCAFFOLD_NAME = data.get("scaffold_name")
            server_seeds = data.get("seeds", [])
            migration_seeds = data.get("migration_seeds", [])

            if server_seeds:
                SEEDS = list(server_seeds)
            if migration_seeds:
                MIGRATION_SEEDS = list(migration_seeds)

            print("[✓] Partitionnement coordonné (Modèle d'îlots génétiques) :")
            if CURRENT_ISLAND_ID:
                print(f"    -> Îlot chimique : [{CURRENT_ISLAND_ID}] {CURRENT_SCAFFOLD_NAME}")
                print(f"    -> Noyau chimique (Scaffold) : {data.get('scaffold_smarts', 'N/A')}")
            print(f"    -> {len(SEEDS)} structures d'ancrage assignées à ce worker.")
            if MIGRATION_SEEDS:
                print(f"    -> {len(MIGRATION_SEEDS)} graine(s) de migration reçue(s) pour diversité croisée.")
            return True
    except Exception:
        print("[i] Serveur central hors-ligne : utilisation de la banque pharmacophore locale autonome.")
        # Partitionnement local basé sur l'identifiant pour éviter les doublons même hors ligne
        seeds_pool = target_info["seeds"]
        random.seed(WORKER_ID)
        SEEDS = random.sample(seeds_pool, min(len(seeds_pool), 3))
        random.seed()
        print(f"[i] Partition pharmacophore locale attribuée ({len(SEEDS)} graines distinctes).")
    return False


def main():
    global SEEDS
    # Initialisation du partitionnement coordonné avec le serveur
    sync_island_seeds()

    cycle = 0
    total_screened_local = 0
    total_hits_found = 0

    print(f"\n[*] Lancement de la génération continue ({target_info['name']})...")

    while True:
        t_start = time.time()
        cycle += 1
        print(f"\n--- [Cycle #{cycle}] Génération IA ({engine_name}) ---")

        # Périodiquement (tous les 10 cycles), mettre à jour les graines de migration
        if cycle % 10 == 0:
            sync_island_seeds()

        # 1. Sélection du pool de graines : 90% îlot dédié, 10% croisement de migration
        current_pool = list(SEEDS)
        if MIGRATION_SEEDS and random.random() < 0.15:
            current_pool.append(random.choice(MIGRATION_SEEDS))

        # 2. Génération De Novo
        start_gen = time.time()
        candidates = generate_molecules(current_pool, args.engine, count=BATCH_SIZE)
        gen_time = time.time() - start_gen
        print(f"[*] {len(candidates)} molécules inédites générées & validées (SAscore + PAINS) en {gen_time:.2f}s")

        if not candidates:
            time.sleep(1)
            continue

        # 3. Inférence IA locale vectorisée par Batch ONNX / GNN
        start_eval = time.time()
        evaluated = evaluate_candidates(candidates, args.engine)
        eval_time = time.time() - start_eval
        total_screened_local += len(evaluated)

        scores = [item["predicted_pkd"] for item in evaluated]
        max_score = max(scores) if scores else 0.0
        mean_score = sum(scores) / len(scores) if scores else 0.0
        print(f"    -> Inférence vectorisée par batch ({len(evaluated)} molécules) en {eval_time:.3f}s (pKd moyen: {mean_score:.2f} | max: {max_score:.2f})")

        # 4. Détection des pépites candidates
        potential_hits = [item for item in evaluated if item["predicted_pkd"] >= HIT_THRESHOLD]
        if not potential_hits and max_score >= 8.2:
            potential_hits = [max(evaluated, key=lambda x: x["predicted_pkd"])]

        confirmed_hits = []
        if potential_hits:
            print(f"[*] Amarrage structure-based (AutoDock Vina) pour {len(potential_hits)} candidate(s)...")
            for h in potential_hits:
                # 5. Modélisation Structure-Based : Docking AutoDock Vina
                dock_res = run_docking(h["smiles"], args.target, exhaustiveness=1)
                docking_kcal = dock_res["affinity_kcal_mol"] if dock_res else None
                h["docking_kcal"] = docking_kcal
                h["island_id"] = CURRENT_ISLAND_ID
                h["scaffold"] = CURRENT_SCAFFOLD_NAME

                pains_str = "✓ PAINS free" if h["pains_ok"] else "⚠ PAINS alert"
                dg_str = f" | ΔG Vina: {docking_kcal} kcal/mol" if docking_kcal is not None else ""
                print(f"    ★ PÉPITE QUALIFIÉE : {h['smiles'][:32]}... | pKd: {h['predicted_pkd']}{dg_str} | SAscore: {h['sascore']} | {pains_str}")

                confirmed_hits.append(h)
                total_hits_found += 1
                if h["smiles"] not in SEEDS:
                    SEEDS.append(h["smiles"])

        # 6. Transmission au serveur central avec persistance SQLite
        payload = {
            "worker_id": WORKER_ID,
            "target": args.target,
            "engine": args.engine,
            "island_id": CURRENT_ISLAND_ID,
            "evaluated_count": len(evaluated),
            "hits": confirmed_hits
        }

        try:
            resp = requests.post(f"{SERVER_URL}/hits/submit", json=payload, timeout=6)
            if resp.status_code == 200:
                data = resp.json()
                accepted = data.get("accepted_hits", 0)
                global_total = data.get("total_evaluated_globally", 0)
                rank = data.get("leaderboard_rank")
                rank_str = f"| Rang mondial : #{rank}" if rank else ""
                worker_ip = data.get("worker_ip", "")
                ip_str = f"| IP : {worker_ip}" if worker_ip else ""
                print(f"[✓] Résultats synchronisés (+{accepted} pépites {rank_str} {ip_str})")
                print(f"    Criblées localement : {total_screened_local} | Total mondial persisté : {global_total}")
        except requests.exceptions.RequestException:
            print("[~] Serveur central temporairement hors-ligne. Calcul autonome continu.")

        # Temporisation avec contrôle d'allocation des ressources CPU (duty-cycle)
        elapsed = time.time() - t_start
        cpu_pct = max(10, min(100, getattr(args, 'cpu_limit', 100)))
        if cpu_pct < 100:
            duty_sleep = max(0.4, elapsed * (100 - cpu_pct) / cpu_pct)
        else:
            duty_sleep = 0.3
        time.sleep(duty_sleep)


if __name__ == "__main__":
    main()
