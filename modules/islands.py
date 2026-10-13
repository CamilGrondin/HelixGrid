import json
import time
from typing import Dict, List, Optional
from rdkit import Chem

# ===========================================================================
# CATALOGUE DES ÎLOTS GÉNÉTIQUES & NOYAUX CHIMIQUES (SCAFFOLDS)
# Modèle d'îlots pour le partitionnement coordonné de l'espace chimique.
# Chaque cible thérapeutique dispose de plusieurs familles de scaffolds
# distinctes distribuées aux workers pour éliminer l'exploration redondante.
# ===========================================================================

ISLAND_CATALOG = {
    "EGFR_T790M": [
        {
            "island_id": "egfr_quinazoline",
            "scaffold_name": "4-Anilinoquinazoline (Famille Gefitinib / Erlotinib)",
            "scaffold_smarts": "c1ccc2ncnc(Nc3ccccc3)c2c1",
            "description": "Noyau bicyclique quinazoline inhibant le site de liaison à l'ATP du domaine kinase EGFR.",
            "seeds": [
                "COc1cc2ncnc(Nc3ccc(F)c(Cl)c3)c2cc1OCC",
                "c1ccc2ncnc(Nc3ccc(Br)cc3)c2c1",
                "COc1cc2c(Nc3ccc(F)c(Cl)c3)ncnc2cc1OC"
            ]
        },
        {
            "island_id": "egfr_pyrim_indole",
            "scaffold_name": "Pyrimidine-Indole Covalent (Famille Osimertinib AZD9291)",
            "scaffold_smarts": "c1ccc(Nc2nccc(-c3cnc4ccccc34)n2)cc1",
            "description": "Inhibiteur de 3e génération sélectif de la mutation de résistance T790M sans cibler l'EGFR sauvage.",
            "seeds": [
                "COc1cc(N(C)CCN(C)C)c(NC(=O)C=C)cc1Nc2nccc(c3cn(C)c4ccccc34)n2",
                "c1ccc(Nc2nccc(-c3cn(C)c4ccccc34)n2)cc1",
                "C=CC(=O)Nc1cc(Nc2nccc(c3cn(C)c4ccccc34)n2)ccc1F"
            ]
        },
        {
            "island_id": "egfr_covalent_anilino",
            "scaffold_name": "Anilinoquinazoline Acrylamide (Famille Afatinib / Dacomitinib)",
            "scaffold_smarts": "C=CC(=O)Nc1cccc2ncnc(Nc3ccccc3)c12",
            "description": "Inhibiteur covalent irréversible de 2e génération réagissant avec Cys797 par addition de Michael.",
            "seeds": [
                "CN(C)CC=CC(=O)Nc1cc2c(Nc3ccc(F)c(Cl)c3)ncnc2cc1OCC",
                "C=CC(=O)Nc1cc2c(Nc3ccc(F)c(Cl)c3)ncnc2cc1OCC",
                "C=CC(=O)Nc1cccc(Nc2ncnc3cc(OC)c(OC)cc23)c1"
            ]
        },
        {
            "island_id": "egfr_diaminopyrim",
            "scaffold_name": "2,4-Diaminopyrimidine / Benzamide (Famille Brigatinib / WZ4002)",
            "scaffold_smarts": "c1ccc(Nc2nccc(Nc3ccccc3)n2)cc1",
            "description": "Scaffold diaminopyrimidine flexible pour contourner les mutations allostériques secondaires (C797S).",
            "seeds": [
                "CN1CCN(CC1)C(=O)C2=CC=C(C=C2)NC(=O)C3=CC=CC=C3",
                "Cc1cc(Nc2nccc(Nc3ccc(OCCN4CCOCC4)cc3)n2)c(C)c(Cl)c1",
                "Nc1ncc(Cl)c(Nc2cccc(S(=O)(=O)C(C)C)c2)n1"
            ]
        }
    ],

    "SARS_CoV_2_Mpro": [
        {
            "island_id": "mpro_peptidomimetic",
            "scaffold_name": "Pyrrolidinone Dipeptide (Famille Nirmatrelvir / Paxlovid)",
            "scaffold_smarts": "O=C(NC(CC1CCNC1=O)C=O)C",
            "description": "Peptidomimétique intégrant une glutamine cyclisée ciblant la poche catalytique Cys145-His41 de la Mpro.",
            "seeds": [
                "CC(C)CC(NC(=O)OCc1ccccc1)C(=O)NC(CC2CCNC2=O)C=O",
                "CC(=O)NC(Cc1ccccc1)C(=O)NC(CC2CCNC2=O)C=O",
                "CC(C)(C)NC(=O)C(NC(=O)C(F)(F)F)C(=O)NC1CC1"
            ]
        },
        {
            "island_id": "mpro_michael_acceptor",
            "scaffold_name": "Michael Acceptor Cinnamoyl (Famille N3 Inhibitor)",
            "scaffold_smarts": "O=C(C=Cc1ccccc1)N",
            "description": "Ester et amides α,β-insaturés capables d'inactiver irréversiblement la cystéine 145 virale.",
            "seeds": [
                "CCOC(=O)C=CC(NC(=O)OCc1ccccc1)C(=O)NC(CC2CCNC2=O)C=O",
                "CC1(C)C2CC2C1(C)NC(=O)C(NC(=O)C(C)C)C(=O)NC3CCCC3"
            ]
        },
        {
            "island_id": "mpro_noncovalent_quinoline",
            "scaffold_name": "Bicyclique Hétéroaryle Carboxamide (Non-covalent Mpro)",
            "scaffold_smarts": "O=C(Nc1ccccc1)c1ccnc2ccccc12",
            "description": "Inhibiteurs non-covalents allostériques ou compétitifs avec profil pharmacocinétique optimisé.",
            "seeds": [
                "O=C(Nc1ccc(F)cc1)c2ccc3ncccc3c2",
                "CC(C)(C)NC(=O)C(NC(=O)C(F)(F)F)C(=O)NC1CC1"
            ]
        }
    ],

    "KRAS_G12D": [
        {
            "island_id": "kras_pyrido_pyrim",
            "scaffold_name": "Pyridopyrimidine Bicyclique (Famille Sotorasib / Switch-II)",
            "scaffold_smarts": "c1ccc2c(n1)ncnc2",
            "description": "Scaffold bicyclique rigide occupant la poche cryptique switch-II de l'oncoprotéine KRAS.",
            "seeds": [
                "Cc1cc(F)cc(c1)n2c(=O)n(c3cc(F)c(F)c(F)c3)c(=O)c4c(N5CCN(C)CC5)ncnc42",
                "Fc1cccc(Nc2ncnc3cc(F)c(F)cc23)c1Cl"
            ]
        },
        {
            "island_id": "kras_triazole_pyrazine",
            "scaffold_name": "Dihydropyridopyrazine Triazole (Famille MRTX1133)",
            "scaffold_smarts": "c1ccc(Nc2ncc3c(n2)N(c2ccccc2)C(=O)N3)cc1",
            "description": "Inhibiteur non-covalent hautement sélectif de la mutation oncogénique Aspartate G12D.",
            "seeds": [
                "Fc1ccc(Nc2ncc3c(n2)N(c2cccc(Cl)c2)C(=O)N3c2ccccc2)cc1F",
                "CN1CCN(CC1)c2ncc(F)c(Nc3ccc(F)cc3)n2"
            ]
        },
        {
            "island_id": "kras_covalent_piperazine",
            "scaffold_name": "Piperazine Acryloyl Covalent",
            "scaffold_smarts": "C=CC(=O)N1CCN(c2ccccc2)CC1",
            "description": "Scaffold armé d'un groupement acrylamide orienté vers les résidus de surface de la poche GTPase.",
            "seeds": [
                "C=CC(=O)N1CCN(CC1)c2ccc3nc(Nc4cccc(F)c4)sc3c2",
                "Fc1ccc(Nc2nccc(N3CCN(CC3)C(=O)C=C)n2)cc1"
            ]
        }
    ],

    "BACE1_Alzheimer": [
        {
            "island_id": "bace1_aminoimidazole",
            "scaffold_name": "Aminoimidazole / Iminopyrimidinone (Famille Verubecestat)",
            "scaffold_smarts": "NC1=NC(=O)N(c2ccccc2)C=N1",
            "description": "Motive bicyclique établissant des liaisons hydrogène clés avec la dyade catalytique Asp32 / Asp228.",
            "seeds": [
                "CN1C(=NC(=N1)N)c2cc(ccc2F)c3cncc(n3)C#N",
                "NC1=NC(=O)N(C)C(=N1)c2cc(ccc2F)Nc3nccc(n3)C(F)(F)F"
            ]
        },
        {
            "island_id": "bace1_aminothiazine",
            "scaffold_name": "Aminothiazine / Dihydrooxazine (Famille Atabecestat)",
            "scaffold_smarts": "c1ccc(c2nc(N)[nH]c2)cc1",
            "description": "Inhibiteur pénétrant la barrière hémato-encéphalique sans toxicité oculaire ni cutanée.",
            "seeds": [
                "Fc1cc(ccc1F)c2nc(N)[nH]c2c3cccnc3",
                "CC1(C)NC(=N)N(C)C1c2cccc(Nc3nccc(F)n3)c2"
            ]
        },
        {
            "island_id": "bace1_acylguanidine",
            "scaffold_name": "Acylguanidine Pyridyle",
            "scaffold_smarts": "NC(=N)Nc1cccnc1",
            "description": "Pharmacophore hétéroaromatique basique modulant le pKa pour réduire l'efflux P-gp cérébral.",
            "seeds": [
                "NC(=N)Nc1cc(F)cc(c1)-c2ccncc2",
                "COc1ccc(Nc2ncc(C(F)(F)F)cc2NC(=N)N)cc1"
            ]
        }
    ],

    "HER2_Breast": [
        {
            "island_id": "her2_anilinoquinoline",
            "scaffold_name": "Anilinoquinoline Cyano (Famille Neratinib)",
            "scaffold_smarts": "c1cc(Nc2c(C#N)cnc3ccccc23)ccc1",
            "description": "Inhibiteur irréversible pan-HER ciblant Cys805 du récepteur tyrosine-kinase HER2.",
            "seeds": [
                "CC1=CN=C(C=N1)C2=CC3=C(C=C2)N=CN=C3NC4=CC=C(C=C4)OC5=CC=CC=C5F",
                "COc1cc2ncnc(Nc3ccc(Br)c(F)c3)c2cc1OCC"
            ]
        },
        {
            "island_id": "her2_furan_quinazoline",
            "scaffold_name": "Furanyl Quinazoline (Famille Lapatinib)",
            "scaffold_smarts": "c1ccc2ncnc(Nc3ccc(OCc4ccco4)cc3)c2c1",
            "description": "Double inhibiteur EGFR/HER2 avec cinétique de dissociation lente (longue durée d'action).",
            "seeds": [
                "CS(=O)(=O)CCNCC1=CC=C(O1)C2=CC3=C(C=C2)N=CN=C3NC4=CC(=C(C=C4)OCC5=CC(=CC=C5)F)Cl",
                "c1ccc(OCc2ccc(Nc3ncnc4cc(OCC5CCCO5)c(OC)cc34)cc2)cc1"
            ]
        },
        {
            "island_id": "her2_thienopyrimidine",
            "scaffold_name": "Thiénopyrimidine Bioisostère",
            "scaffold_smarts": "c1sc2ncnc(Nc3ccccc3)c2c1",
            "description": "Bioisostère soufré du noyau quinazoline offrant une sélectivité accrue sur ERBB2.",
            "seeds": [
                "c1sc2ncnc(Nc3ccc(OCc4ccccn4)c(Cl)c3)c2c1",
                "Cc1nnc(Nc2ncnc3scc(C#N)c23)s1"
            ]
        }
    ]
}


def sync_islands_to_db(db_manager):
    """Initialise ou synchronise le catalogue d'îlots dans la base de données."""
    now = time.time()
    with db_manager.get_connection() as conn:
        cur = conn.cursor()
        for target, islands in ISLAND_CATALOG.items():
            for isl in islands:
                cur.execute("""
                    INSERT INTO islands (
                        island_id, target, scaffold_name, scaffold_smarts,
                        description, seeds_json, updated_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?)
                    ON CONFLICT(island_id) DO UPDATE SET
                        scaffold_name = excluded.scaffold_name,
                        scaffold_smarts = excluded.scaffold_smarts,
                        description = excluded.description,
                        seeds_json = excluded.seeds_json,
                        updated_at = excluded.updated_at;
                """, (
                    isl["island_id"], target, isl["scaffold_name"],
                    isl["scaffold_smarts"], isl["description"],
                    json.dumps(isl["seeds"]), now
                ))
        conn.commit()


def assign_island_to_worker(db_manager, target: str, worker_id: str, worker_ip: str) -> dict:
    """
    Algorithme de partitionnement coordonné :
    Attribue un îlot chimique distinct à chaque worker afin d'éliminer la redondance
    d'exploration (modèle d'îlots génétiques).
    """
    # Vérifier si la cible existe dans le catalogue, sinon EGFR par défaut
    target_key = target if target in ISLAND_CATALOG else "EGFR_T790M"
    available_islands = ISLAND_CATALOG[target_key]

    # Vérifier si le worker a déjà un îlot attribué
    worker_info = db_manager.get_worker(worker_id)
    assigned_island_id = None
    if worker_info and worker_info.get("assigned_island") and worker_info.get("target") == target_key:
        assigned_island_id = worker_info["assigned_island"]

    chosen_island = None
    if assigned_island_id:
        chosen_island = next((isl for isl in available_islands if isl["island_id"] == assigned_island_id), None)

    if chosen_island is None:
        # Équilibrage de charge : trouver l'îlot le moins exploré ou avec le moins de workers actifs
        now = time.time()
        with db_manager.get_connection() as conn:
            cur = conn.cursor()
            cur.execute("""
                SELECT assigned_island, COUNT(*) as worker_cnt
                FROM workers
                WHERE target = ? AND (? - last_seen) < 120
                GROUP BY assigned_island;
            """, (target_key, now))
            counts = {row["assigned_island"]: row["worker_cnt"] for row in cur.fetchall()}

        # Trier les îlots par nombre de workers actifs croissant
        sorted_islands = sorted(available_islands, key=lambda isl: counts.get(isl["island_id"], 0))
        chosen_island = sorted_islands[0]

    # Mise à jour du worker dans la base
    db_manager.register_or_update_worker(
        worker_id=worker_id,
        worker_ip=worker_ip,
        target=target_key,
        assigned_island=chosen_island["island_id"],
        assigned_scaffold=chosen_island["scaffold_name"]
    )

    # Récupérer les "graines de migration" : meilleures découvertes d'autres îlots pour le croisement génétique
    migration_seeds = []
    with db_manager.get_connection() as conn:
        cur = conn.cursor()
        cur.execute("""
            SELECT smiles, score FROM molecules
            WHERE target = ? AND (island_id != ? OR island_id IS NULL)
            ORDER BY score DESC LIMIT 2;
        """, (target_key, chosen_island["island_id"]))
        for r in cur.fetchall():
            migration_seeds.append(r["smiles"])

    return {
        "target": target_key,
        "worker_id": worker_id,
        "island_id": chosen_island["island_id"],
        "scaffold_name": chosen_island["scaffold_name"],
        "scaffold_smarts": chosen_island["scaffold_smarts"],
        "description": chosen_island["description"],
        "seeds": list(chosen_island["seeds"]),
        "migration_seeds": migration_seeds,
        "total_target_islands": len(available_islands)
    }


def get_islands_status(db_manager, target: Optional[str] = None) -> List[dict]:
    """Retourne l'état d'exploration de chaque îlot génétique."""
    now = time.time()
    with db_manager.get_connection() as conn:
        cur = conn.cursor()
        query = """
            SELECT 
                i.island_id, i.target, i.scaffold_name, i.scaffold_smarts,
                i.description, i.total_hits, i.best_score,
                COUNT(w.worker_id) as active_workers
            FROM islands i
            LEFT JOIN workers w ON w.assigned_island = i.island_id AND (? - w.last_seen) < 120
        """
        params = [now]
        if target:
            query += " WHERE i.target = ?"
            params.append(target)
        query += " GROUP BY i.island_id ORDER BY i.target, i.island_id;"

        cur.execute(query, params)
        return [dict(row) for row in cur.fetchall()]
