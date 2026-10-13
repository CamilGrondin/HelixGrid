import os
import time
import sqlite3
from typing import List, Dict, Optional, Set
from rdkit import Chem
from rdkit.Chem import AllChem, Descriptors
from rdkit.Chem.Scaffolds import MurckoScaffold

DEFAULT_DB_PATH = os.getenv("HELIXGRID_DB_PATH", "helixgrid.db")


def generate_sdf_3d(smiles: str, title: Optional[str] = None) -> Optional[str]:
    """
    Génère les coordonnées 3D d'une molécule (champ de force MMFF94)
    et retourne le bloc MolBlock/SDF au format standard.
    """
    try:
        mol = Chem.MolFromSmiles(smiles)
        if mol is None:
            return None
        mol.SetProp("_Name", title or smiles)
        mol_h = Chem.AddHs(mol)
        params = AllChem.ETKDGv3()
        params.randomSeed = 42
        embed_status = AllChem.EmbedMolecule(mol_h, params)
        if embed_status != 0:
            embed_status = AllChem.EmbedMolecule(mol_h, randomSeed=42)
        if embed_status == 0:
            try:
                AllChem.MMFFOptimizeMolecule(mol_h, maxIters=150)
            except Exception:
                pass
        return Chem.MolToMolBlock(mol_h)
    except Exception as e:
        print(f"[!] Erreur génération coordonnées 3D SDF pour {smiles}: {e}")
        return None


def extract_murcko_scaffold(smiles: str) -> str:
    """Extrait le scaffold de Bemis-Murcko pour une molécule."""
    try:
        mol = Chem.MolFromSmiles(smiles)
        if mol:
            scaffold_mol = MurckoScaffold.GetScaffoldForMol(mol)
            return Chem.MolToSmiles(scaffold_mol)
    except Exception:
        pass
    return ""


class DatabaseManager:
    """
    Gestionnaire de persistance SQLite haute performance pour HelixGrid.
    Gère les molécules candidates, structures SDF 3D, adresses IP des bénévoles,
    scores pKd/docking et statistiques d'exploration.
    """

    def __init__(self, db_path: str = DEFAULT_DB_PATH):
        self.db_path = db_path
        self.init_db()

    def get_connection(self) -> sqlite3.Connection:
        """Ouvre une connexion SQLite thread-safe avec mode WAL activé."""
        conn = sqlite3.connect(self.db_path, timeout=15.0)
        conn.row_factory = sqlite3.Row
        conn.execute("PRAGMA journal_mode=WAL;")
        conn.execute("PRAGMA synchronous=NORMAL;")
        conn.execute("PRAGMA busy_timeout=5000;")
        return conn

    def init_db(self):
        """Initialise le schéma de la base de données avec index optimisés."""
        with self.get_connection() as conn:
            cur = conn.cursor()

            # Table des molécules découvertes
            cur.execute("""
                CREATE TABLE IF NOT EXISTS molecules (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    smiles TEXT UNIQUE NOT NULL,
                    score REAL NOT NULL,
                    docking_kcal REAL,
                    mw REAL,
                    logp REAL,
                    sascore REAL,
                    pains_ok INTEGER DEFAULT 1,
                    brenk_ok INTEGER DEFAULT 1,
                    sdf_3d TEXT,
                    scaffold TEXT,
                    island_id TEXT,
                    discovered_by TEXT,
                    worker_ip TEXT,
                    target TEXT,
                    engine TEXT,
                    discovered_at REAL NOT NULL
                );
            """)

            # Table des workers connectés
            cur.execute("""
                CREATE TABLE IF NOT EXISTS workers (
                    worker_id TEXT PRIMARY KEY,
                    worker_ip TEXT,
                    target TEXT,
                    engine TEXT,
                    assigned_island TEXT,
                    assigned_scaffold TEXT,
                    total_evaluated INTEGER DEFAULT 0,
                    hits_count INTEGER DEFAULT 0,
                    first_seen REAL NOT NULL,
                    last_seen REAL NOT NULL
                );
            """)

            # Table des îlots génétiques (scaffolds)
            cur.execute("""
                CREATE TABLE IF NOT EXISTS islands (
                    island_id TEXT PRIMARY KEY,
                    target TEXT NOT NULL,
                    scaffold_name TEXT NOT NULL,
                    scaffold_smarts TEXT NOT NULL,
                    description TEXT,
                    seeds_json TEXT,
                    total_hits INTEGER DEFAULT 0,
                    best_score REAL DEFAULT 0.0,
                    updated_at REAL NOT NULL
                );
            """)

            # Table de traçabilité des soumissions de calcul
            cur.execute("""
                CREATE TABLE IF NOT EXISTS submissions (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    worker_id TEXT NOT NULL,
                    worker_ip TEXT,
                    target TEXT,
                    engine TEXT,
                    island_id TEXT,
                    evaluated_count INTEGER NOT NULL,
                    hits_count INTEGER NOT NULL,
                    submitted_at REAL NOT NULL
                );
            """)

            # Index pour requêtes rapides
            cur.execute("CREATE INDEX IF NOT EXISTS idx_molecules_score ON molecules(score DESC);")
            cur.execute("CREATE INDEX IF NOT EXISTS idx_molecules_smiles ON molecules(smiles);")
            cur.execute("CREATE INDEX IF NOT EXISTS idx_molecules_target ON molecules(target);")
            cur.execute("CREATE INDEX IF NOT EXISTS idx_molecules_island ON molecules(island_id);")
            cur.execute("CREATE INDEX IF NOT EXISTS idx_molecules_worker ON molecules(discovered_by);")
            cur.execute("CREATE INDEX IF NOT EXISTS idx_workers_last_seen ON workers(last_seen);")

            conn.commit()

    def get_known_smiles(self) -> Set[str]:
        """Retourne l'ensemble des SMILES déjà enregistrés pour un dédoublonnage instantané."""
        with self.get_connection() as conn:
            cur = conn.cursor()
            cur.execute("SELECT smiles FROM molecules;")
            return {row["smiles"] for row in cur.fetchall()}

    def get_worker(self, worker_id: str) -> Optional[dict]:
        """Récupère les informations d'un worker."""
        with self.get_connection() as conn:
            cur = conn.cursor()
            cur.execute("SELECT * FROM workers WHERE worker_id = ?;", (worker_id,))
            row = cur.fetchone()
            return dict(row) if row else None

    def register_or_update_worker(
        self,
        worker_id: str,
        worker_ip: str,
        target: Optional[str] = None,
        engine: Optional[str] = None,
        assigned_island: Optional[str] = None,
        assigned_scaffold: Optional[str] = None,
        evaluated_add: int = 0,
        hits_add: int = 0
    ) -> dict:
        """Enregistre ou met à jour l'activité d'un worker bénévole."""
        now = time.time()
        with self.get_connection() as conn:
            cur = conn.cursor()
            cur.execute("SELECT * FROM workers WHERE worker_id = ?;", (worker_id,))
            existing = cur.fetchone()

            if existing is None:
                cur.execute("""
                    INSERT INTO workers (
                        worker_id, worker_ip, target, engine,
                        assigned_island, assigned_scaffold,
                        total_evaluated, hits_count, first_seen, last_seen
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
                """, (
                    worker_id, worker_ip, target, engine,
                    assigned_island, assigned_scaffold,
                    evaluated_add, hits_add, now, now
                ))
            else:
                new_island = assigned_island or existing["assigned_island"]
                new_scaffold = assigned_scaffold or existing["assigned_scaffold"]
                new_target = target or existing["target"]
                new_engine = engine or existing["engine"]

                cur.execute("""
                    UPDATE workers SET
                        worker_ip = ?,
                        target = ?,
                        engine = ?,
                        assigned_island = ?,
                        assigned_scaffold = ?,
                        total_evaluated = total_evaluated + ?,
                        hits_count = hits_count + ?,
                        last_seen = ?
                    WHERE worker_id = ?;
                """, (
                    worker_ip, new_target, new_engine,
                    new_island, new_scaffold,
                    evaluated_add, hits_add, now, worker_id
                ))

            conn.commit()
            return self.get_worker(worker_id)

    def record_submission(
        self,
        worker_id: str,
        worker_ip: str,
        evaluated_count: int,
        hits: List[dict],
        target: Optional[str] = None,
        engine: Optional[str] = None,
        island_id: Optional[str] = None
    ) -> int:
        """
        Enregistre une soumission de batch, persiste les nouvelles molécules avec
        leurs structures 3D SDF et met à jour les statistiques workers et îles.
        """
        now = time.time()
        accepted_count = 0

        with self.get_connection() as conn:
            cur = conn.cursor()

            # Mise à jour des compteurs du worker
            self.register_or_update_worker(
                worker_id=worker_id,
                worker_ip=worker_ip,
                target=target,
                engine=engine,
                assigned_island=island_id,
                evaluated_add=evaluated_count,
                hits_add=0  # Mis à jour après insertion
            )

            for hit in hits:
                smiles = hit.get("smiles")
                if not smiles:
                    continue

                # Vérifier si déjà présent
                cur.execute("SELECT id, score FROM molecules WHERE smiles = ?;", (smiles,))
                row = cur.fetchone()

                score = float(hit.get("predicted_pkd", hit.get("score", 6.0)))
                docking_kcal = hit.get("docking_kcal")
                mw = hit.get("mw")
                logp = hit.get("logp")
                sascore = hit.get("sascore")
                pains_ok = 1 if hit.get("pains_ok", True) else 0
                brenk_ok = 1 if hit.get("brenk_ok", True) else 0

                scaffold = hit.get("scaffold") or extract_murcko_scaffold(smiles)
                hit_island = hit.get("island_id") or island_id

                if row is None:
                    # Génération de la conformation 3D SDF
                    sdf_3d = hit.get("sdf_3d") or generate_sdf_3d(smiles, title=f"HelixGrid_{smiles[:20]}")

                    cur.execute("""
                        INSERT INTO molecules (
                            smiles, score, docking_kcal, mw, logp, sascore,
                            pains_ok, brenk_ok, sdf_3d, scaffold, island_id,
                            discovered_by, worker_ip, target, engine, discovered_at
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
                    """, (
                        smiles, score, docking_kcal, mw, logp, sascore,
                        pains_ok, brenk_ok, sdf_3d, scaffold, hit_island,
                        worker_id, worker_ip, target, engine, now
                    ))
                    accepted_count += 1
                else:
                    # Mise à jour du score si meilleur
                    if score > row["score"]:
                        cur.execute("""
                            UPDATE molecules SET
                                score = ?, docking_kcal = COALESCE(?, docking_kcal),
                                discovered_by = ?, worker_ip = ?, discovered_at = ?
                            WHERE id = ?;
                        """, (score, docking_kcal, worker_id, worker_ip, now, row["id"]))

            # Incrémenter les hits du worker
            if accepted_count > 0:
                cur.execute("""
                    UPDATE workers SET hits_count = hits_count + ? WHERE worker_id = ?;
                """, (accepted_count, worker_id))

                if hit_island:
                    cur.execute("""
                        UPDATE islands SET
                            total_hits = total_hits + ?,
                            best_score = MAX(best_score, ?),
                            updated_at = ?
                        WHERE island_id = ?;
                    """, (accepted_count, max((h.get("predicted_pkd", 0.0) for h in hits), default=0.0), now, hit_island))

            # Historisation de la soumission
            cur.execute("""
                INSERT INTO submissions (
                    worker_id, worker_ip, target, engine, island_id,
                    evaluated_count, hits_count, submitted_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?);
            """, (
                worker_id, worker_ip, target, engine,
                island_id, evaluated_count, accepted_count, now
            ))

            conn.commit()

        return accepted_count

    def get_leaderboard(
        self,
        target: Optional[str] = None,
        island_id: Optional[str] = None,
        limit: int = 50
    ) -> List[dict]:
        """Récupère le classement des meilleures molécules découvertes."""
        with self.get_connection() as conn:
            cur = conn.cursor()
            query = "SELECT * FROM molecules WHERE 1=1"
            params = []

            if target:
                query += " AND (target = ? OR target IS NULL)"
                params.append(target)
            if island_id:
                query += " AND island_id = ?"
                params.append(island_id)

            query += " ORDER BY score DESC LIMIT ?;"
            params.append(limit)

            cur.execute(query, params)
            rows = cur.fetchall()

            result = []
            for r in rows:
                item = dict(r)
                # Formater les booléens
                item["pains_ok"] = bool(item.get("pains_ok", 1))
                item["brenk_ok"] = bool(item.get("brenk_ok", 1))
                result.append(item)
            return result

    def get_worker_rank(self, worker_id: str) -> Optional[int]:
        """Calcule le rang d'un worker dans le classement mondial des pépites."""
        with self.get_connection() as conn:
            cur = conn.cursor()
            cur.execute("""
                SELECT ROW_NUMBER() OVER (ORDER BY score DESC) as rank, discovered_by
                FROM molecules;
            """)
            for row in cur.fetchall():
                if row["discovered_by"] == worker_id:
                    return int(row["rank"])
            return None

    def get_stats(self, target: Optional[str] = None) -> dict:
        """Calcule les statistiques globales du réseau persistant."""
        now = time.time()
        with self.get_connection() as conn:
            cur = conn.cursor()

            # Total de molécules évaluées par le réseau
            cur.execute("SELECT COALESCE(SUM(total_evaluated), 0) as total FROM workers;")
            total_screened = int(cur.fetchone()["total"])

            # Total de hits uniques
            query_hits = "SELECT COUNT(*) as count, MAX(score) as best FROM molecules"
            params = []
            if target:
                query_hits += " WHERE target = ? OR target IS NULL"
                params.append(target)
            cur.execute(query_hits, params)
            hit_row = cur.fetchone()
            total_hits = int(hit_row["count"])
            best_score = float(hit_row["best"]) if hit_row["best"] is not None else 0.0

            # Nombre de workers actifs (vus dans les 60 dernières secondes)
            cur.execute("SELECT COUNT(*) as active FROM workers WHERE (? - last_seen) < 60;", (now,))
            active_workers = int(cur.fetchone()["active"])

            # Nombre d'îlots chimiques
            cur.execute("SELECT COUNT(*) as count FROM islands;")
            islands_count = int(cur.fetchone()["count"])

            # Décompte d'énergie & calcul par pathologie (Cancer vs Neurodégénérescence vs Antiviral)
            cur.execute("""
                SELECT 
                    COALESCE(SUM(CASE WHEN target IN ('EGFR_T790M', 'KRAS_G12D', 'HER2_Breast') THEN total_evaluated ELSE 0 END), 0) as cancer_screened,
                    COALESCE(SUM(CASE WHEN target = 'BACE1_Alzheimer' THEN total_evaluated ELSE 0 END), 0) as neuro_screened,
                    COALESCE(SUM(CASE WHEN target = 'SARS_CoV_2_Mpro' THEN total_evaluated ELSE 0 END), 0) as antiviral_screened
                FROM workers;
            """)
            breakdown_row = cur.fetchone()
            cancer_eval = int(breakdown_row["cancer_screened"] or 0)
            neuro_eval = int(breakdown_row["neuro_screened"] or 0)
            antiviral_eval = int(breakdown_row["antiviral_screened"] or 0)

            return {
                "target": target or "EGFR_T790M",
                "total_molecules_screened": total_screened,
                "total_unique_hits": total_hits,
                "best_pkd_score": best_score,
                "active_workers": active_workers,
                "total_islands": islands_count,
                "breakdown": {
                    "cancer_screened": cancer_eval,
                    "neuro_screened": neuro_eval,
                    "antiviral_screened": antiviral_eval
                }
            }

    def get_volunteers_leaderboard(self, limit: int = 20) -> List[dict]:
        """Récupère le classement des meilleurs bénévoles par molécules évaluées et pépites."""
        with self.get_connection() as conn:
            cur = conn.cursor()
            cur.execute("""
                SELECT worker_id, total_evaluated, hits_count, target, engine, first_seen, last_seen
                FROM workers
                ORDER BY hits_count DESC, total_evaluated DESC
                LIMIT ?;
            """, (limit,))
            rows = cur.fetchall()
            return [dict(r) for r in rows]

    def get_molecule_sdf(self, identifier: str) -> Optional[str]:
        """Récupère la structure SDF 3D d'une molécule par ID ou SMILES (brut ou canonique), avec génération à la volée si absente."""
        with self.get_connection() as conn:
            cur = conn.cursor()
            if identifier.isdigit():
                cur.execute("SELECT sdf_3d, smiles FROM molecules WHERE id = ?;", (int(identifier),))
            else:
                can_smi = identifier
                try:
                    m = Chem.MolFromSmiles(identifier)
                    if m:
                        can_smi = Chem.MolToSmiles(m)
                except Exception:
                    pass
                cur.execute("SELECT sdf_3d, smiles FROM molecules WHERE smiles = ? OR smiles = ?;", (identifier, can_smi))
            row = cur.fetchone()
            if row and row["sdf_3d"]:
                return row["sdf_3d"]

            # Si non trouvée en base mais qu'il s'agit d'un SMILES valide, générer les coordonnées 3D à la volée
            if not identifier.isdigit():
                generated = generate_sdf_3d(identifier, title=f"HelixGrid_{identifier[:20]}")
                if generated:
                    return generated

            return None

    def export_all_sdf(self, target: Optional[str] = None, limit: int = 100) -> str:
        """Exporte les meilleures structures 3D au format multi-molécule SDF standard."""
        with self.get_connection() as conn:
            cur = conn.cursor()
            query = "SELECT sdf_3d, smiles, score FROM molecules WHERE sdf_3d IS NOT NULL"
            params = []
            if target:
                query += " AND (target = ? OR target IS NULL)"
                params.append(target)
            query += " ORDER BY score DESC LIMIT ?;"
            params.append(limit)

            cur.execute(query, params)
            rows = cur.fetchall()

            sdf_blocks = []
            for r in rows:
                block = r["sdf_3d"].strip()
                if block:
                    sdf_blocks.append(block + "\n$$$$\n")

            return "".join(sdf_blocks)
