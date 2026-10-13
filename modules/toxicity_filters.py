import os
import sys
from rdkit import Chem, RDConfig
from rdkit.Chem import Descriptors, Crippen, rdMolDescriptors
from rdkit.Chem.FilterCatalog import FilterCatalog, FilterCatalogParams

# Configuration du SA_Score (Synthetic Accessibility Score de RDKit)
sa_score_dir = os.path.join(getattr(RDConfig, 'RDContribDir', ''), 'SA_Score')
if os.path.exists(sa_score_dir) and sa_score_dir not in sys.path:
    sys.path.append(sa_score_dir)

try:
    import sascorer
    HAS_SASCORER = True
except ImportError:
    HAS_SASCORER = False

# Initialisation des catalogues de filtres PAINS et Brenk
_pains_params = FilterCatalogParams()
_pains_params.AddCatalog(FilterCatalogParams.FilterCatalogs.PAINS)
_pains_catalog = FilterCatalog(_pains_params)

_brenk_params = FilterCatalogParams()
_brenk_params.AddCatalog(FilterCatalogParams.FilterCatalogs.BRENK)
_brenk_catalog = FilterCatalog(_brenk_params)


def compute_sa_score(mol) -> float:
    """
    Calcule le score d'accessibilité synthétique (SAscore) de RDKit (Ertl & Schuffenhauer).
    Échelle de 1 (très facile à synthétiser) à 10 (très complexe / non synthétisable).
    """
    if mol is None:
        return 10.0
    if HAS_SASCORER:
        try:
            return round(float(sascorer.calculateScore(mol)), 2)
        except Exception:
            return 10.0
    return 3.0  # Valeur par défaut neutre si indisponible


def check_pains(mol) -> tuple[bool, list[str]]:
    """
    Vérifie la présence de motifs PAINS (Pan-Assay Interference Compounds).
    Retourne (est_valide_sans_pains, liste_alertes).
    """
    if mol is None:
        return False, ["Molécule invalide"]
    matches = _pains_catalog.GetMatches(mol)
    alerts = [m.GetDescription() for m in matches]
    return len(alerts) == 0, alerts


def check_brenk(mol) -> tuple[bool, list[str]]:
    """
    Vérifie les alertes de Brenk (groupements réactifs indésirables, métabolites instables).
    Retourne (est_valide_sans_brenk, liste_alertes).
    """
    if mol is None:
        return False, ["Molécule invalide"]
    matches = _brenk_catalog.GetMatches(mol)
    alerts = [m.GetDescription() for m in matches]
    return len(alerts) == 0, alerts


def compute_admet_profile(mol_or_smiles, max_sascore=5.0) -> dict:
    """
    Évalue le profil complet ADMET, SAscore, PAINS, Brenk et Lipinski d'une molécule.
    """
    if isinstance(mol_or_smiles, str):
        mol = Chem.MolFromSmiles(mol_or_smiles)
    else:
        mol = mol_or_smiles

    if mol is None:
        return {
            "valid": False,
            "lipinski_ok": False,
            "bbb_ok": False,
            "pains_ok": False,
            "brenk_ok": False,
            "sascore": 10.0,
            "synthesizable": False,
            "drug_candidate": False,
            "reasons": ["Structure chimique invalide"]
        }

    # 1. Propriétés physico-chimiques
    mw = round(Descriptors.MolWt(mol), 1)
    logp = round(Crippen.MolLogP(mol), 2)
    tpsa = round(Descriptors.TPSA(mol), 1)
    hbd = int(rdMolDescriptors.CalcNumHBD(mol))
    hba = int(rdMolDescriptors.CalcNumHBA(mol))
    rot_bonds = int(Descriptors.NumRotatableBonds(mol))

    # Règle des 5 de Lipinski
    lipinski_ok = (mw <= 500) and (logp <= 5.0) and (hbd <= 5) and (hba <= 10) and (rot_bonds <= 10)

    # Perméabilité Barrière Hémato-Encéphalique (BHE / BBB)
    bbb_ok = (tpsa <= 90.0) and (mw <= 480) and (1.0 <= logp <= 4.5)

    # 2. Accessibilité Synthétique (SAscore)
    sascore = compute_sa_score(mol)
    synthesizable = sascore <= max_sascore

    # 3. Filtres PAINS & Brenk
    pains_ok, pains_alerts = check_pains(mol)
    brenk_ok, brenk_alerts = check_brenk(mol)

    reasons = []
    if not lipinski_ok:
        reasons.append(f"Lipinski échoué (MW={mw}, LogP={logp}, HBD={hbd}, HBA={hba}, Rot={rot_bonds})")
    if not synthesizable:
        reasons.append(f"Synthèse trop complexe (SAscore={sascore} > {max_sascore})")
    if not pains_ok:
        reasons.append(f"Alerte PAINS ({', '.join(pains_alerts)})")
    if not brenk_ok:
        reasons.append(f"Alerte Brenk ({', '.join(brenk_alerts)})")

    # Éligibilité globale comme candidat médicament
    drug_candidate = lipinski_ok and synthesizable and pains_ok

    return {
        "valid": True,
        "mw": mw,
        "logp": logp,
        "tpsa": tpsa,
        "hbd": hbd,
        "hba": hba,
        "rot_bonds": rot_bonds,
        "lipinski_ok": lipinski_ok,
        "bbb_ok": bbb_ok,
        "sascore": sascore,
        "synthesizable": synthesizable,
        "pains_ok": pains_ok,
        "pains_alerts": pains_alerts,
        "brenk_ok": brenk_ok,
        "brenk_alerts": brenk_alerts,
        "drug_candidate": drug_candidate,
        "reasons": reasons
    }
