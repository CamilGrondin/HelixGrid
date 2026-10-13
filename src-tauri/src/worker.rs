use chematic::core::{AtomIdx, BondIdx, BondOrder, Element, Molecule, MoleculeBuilder};
use chematic::fp::ecfp::ecfp4_rdkit_invariants;
use chematic::smiles::{canonical_smiles, parse};
use ndarray::Array2;
use ort::session::Session;
use rand::prelude::IndexedRandom;
use rand::RngExt;
use reqwest::Client;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager, Window};
use tokio::time::sleep;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};

const DEFAULT_SERVER_URL: &str = "https://srv1091727.hstgr.cloud";
const BATCH_SIZE: usize = 250;

pub fn get_server_url() -> String {
    std::env::var("HELIXGRID_SERVER_URL")
        .unwrap_or_else(|_| DEFAULT_SERVER_URL.to_string())
}

#[derive(Serialize, Clone, Debug)]
pub struct MoleculeDiscoveryPayload {
    pub smiles: String,
    pub score: f32,
    pub mw: f32,
    pub logp: f32,
    pub sascore: f32,
    pub pains_ok: bool,
    pub brenk_ok: bool,
    pub is_hit: bool,
    pub target: String,
    pub engine: String,
    pub scaffold: Option<String>,
    pub island_id: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct WorkerMetricsPayload {
    pub cycle: usize,
    pub screened_batch: usize,
    pub screened_total: usize,
    pub total_hits_local: usize,
    pub target: String,
    pub cpu_limit: u32,
    pub idle_only: bool,
    pub is_idle: bool,
}

#[derive(Clone, Debug)]
pub struct TargetConfig {
    pub name: &'static str,
    pub pdb: &'static str,
    pub model_file: &'static str,
    pub threshold: f32,
    pub seeds: &'static [&'static str],
    pub center: (f32, f32, f32),
    pub size: (f32, f32, f32),
}

pub fn get_target_config(target: &str) -> TargetConfig {
    match target {
        "SARS_CoV_2_Mpro" => TargetConfig {
            name: "Covid-19 / Antiviral (SARS-CoV-2 Mpro)",
            pdb: "7BQ7",
            model_file: "affinity_model.onnx",
            threshold: 8.4,
            seeds: &[
                "CC(C)(C)NC(=O)C(NC(=O)C(F)(F)F)C(=O)NC1CC1",
                "CC1(C)C2CC2C1(C)NC(=O)C(NC(=O)C(C)C)C(=O)NC3CCCC3",
                "CC(C)CC(NC(=O)OCc1ccccc1)C(=O)NC(CC2CCNC2=O)C=O",
                "CC(=O)NC(Cc1ccccc1)C(=O)NC(CC2CCNC2=O)C=O",
                "CCOC(=O)C=CC(NC(=O)OCc1ccccc1)C(=O)NC(CC2CCNC2=O)C=O",
            ],
            center: (-84.8, 15.3, 28.1),
            size: (22.0, 22.0, 22.0),
        },
        "KRAS_G12D" => TargetConfig {
            name: "Cancer Pancréas & Côlon (KRAS G12D)",
            pdb: "8T5V",
            model_file: "affinity_model.onnx",
            threshold: 8.5,
            seeds: &[
                "Cc1cc(F)cc(c1)n2c(=O)n(c3cc(F)c(F)c(F)c3)c(=O)c4c(N5CCN(C)CC5)ncnc42",
                "Fc1ccc(Nc2ncc3c(n2)N(c2cccc(Cl)c2)C(=O)N3c2ccccc2)cc1F",
                "CN1CCN(CC1)c2ncc(F)c(Nc3ccc(F)cc3)n2",
                "Fc1cccc(Nc2ncnc3cc(F)c(F)cc23)c1Cl",
            ],
            center: (20.8, 16.8, -17.2),
            size: (20.0, 20.0, 20.0),
        },
        "BACE1_Alzheimer" => TargetConfig {
            name: "Maladie d'Alzheimer (BACE-1 Protéase)",
            pdb: "1FKN",
            model_file: "affinity_model.onnx",
            threshold: 8.5,
            seeds: &[
                "CN1C(=NC(=N1)N)c2cc(ccc2F)c3cncc(n3)C#N",
                "NC1=NC(=O)N(C)C(=N1)c2cc(ccc2F)Nc3nccc(n3)C(F)(F)F",
                "Fc1cc(ccc1F)c2nc(N)[nH]c2c3cccnc3",
                "CC1(C)NC(=N)N(C)C1c2cccc(Nc3nccc(F)n3)c2",
            ],
            center: (26.3, 18.1, 20.8),
            size: (22.0, 22.0, 22.0),
        },
        "HER2_Breast" => TargetConfig {
            name: "Cancer du Sein (HER2 / ERBB2)",
            pdb: "3PP0",
            model_file: "affinity_model.onnx",
            threshold: 8.6,
            seeds: &[
                "CC1=CN=C(C=N1)C2=CC3=C(C=C2)N=CN=C3NC4=CC=C(C=C4)OC5=CC=CC=C5F",
                "CS(=O)(=O)CCNCC1=CC=C(O1)C2=CC3=C(C=C2)N=CN=C3NC4=CC(=C(C=C4)OCC5=CC(=CC=C5)F)Cl",
                "COc1cc2ncnc(Nc3ccc(Br)c(F)c3)c2cc1OCC",
            ],
            center: (25.9, 30.6, 7.6),
            size: (20.0, 20.0, 20.0),
        },
        "Parkinson_LRRK2" => TargetConfig {
            name: "Maladie de Parkinson (Kinase LRRK2 G2019S)",
            pdb: "3PP0",
            model_file: "model_parkinson_lrrk2.onnx",
            threshold: 8.5,
            seeds: &[
                "COc1cc(N2CCC(N3CCN(C)CC3)CC2)ccc1Nc1ncc(Cl)c(Nc2cccc(S(=O)(=O)C(C)C)c2)n1",
                "COc1cc(Nc2ncc(F)c(Nc3ccc4c(n3)NC(=O)C(C)(C)O4)n2)ccc1N1CCC(O)CC1",
                "CS(=O)(=O)c1cccc(Nc2ncc(Cl)c(Nc3ccc(N4CCC(N5CCN(C)CC5)CC4)cc3OC)n2)c1",
            ],
            center: (25.9, 30.6, 7.6),
            size: (20.0, 20.0, 20.0),
        },
        "PKNB_Tuberculosis" => TargetConfig {
            name: "Antibiorésistance / Tuberculose (Kinase PknB)",
            pdb: "8T5V",
            model_file: "model_antibiotic_pknb.onnx",
            threshold: 8.4,
            seeds: &[
                "CCn1c(-c2nonc2N)nc2c(C#CC(C)(C)O)ncc(OCC3CCCNCC3)c21",
                "CCN(CC)CCNC(=O)c1c(C)[nH]c(C=C2C(=O)Nc3ccc(F)c23)c1C",
                "Cc1c(C(=O)NCC2CCCNCC2)[nH]c(C=C3C(=O)Nc4ccc(F)c34)c1C",
            ],
            center: (20.8, 16.8, -17.2),
            size: (20.0, 20.0, 20.0),
        },
        _ => TargetConfig {
            name: "Cancer du Poumon (EGFR T790M Résistant)",
            pdb: "2JIT",
            model_file: "affinity_model.onnx",
            threshold: 8.5,
            seeds: &[
                "COc1cc2ncnc(Nc3ccc(F)c(Cl)c3)c2cc1OCC",
                "CN(C)CC=CC(=O)Nc1cc2c(Nc3ccc(F)c(Cl)c3)ncnc2cc1OCC",
                "COc1cc(N(C)CCN(C)C)c(NC(=O)C=C)cc1Nc2nccc(c3cn(C)c4ccccc34)n2",
                "CN1CCN(CC1)C(=O)C2=CC=C(C=C2)NC(=O)C3=CC=CC=C3",
                "c1ccc2ncnc(Nc3ccc(Br)cc3)c2c1",
                "COc1cc2c(Nc3ccc(F)c(Cl)c3)ncnc2cc1OC",
            ],
            center: (-5.5, 22.5, 26.0),
            size: (20.0, 20.0, 20.0),
        },
    }
}

pub fn get_engine_name(engine_id: &str) -> &'static str {
    match engine_id {
        "generative_evolution" => "IA Générative De Novo (SMARTS + Évolution Rust Native)",
        "supercharged_ai" => "IA Superchargée (3D Pocket Cross-Attention + GFlowNet + RAG)",
        "chembl_rl_agent" => "RL Policy + MolecularGPT (ChEMBL 37)",
        "selfies_dmpnn" => "IA Générative Pure Rust + Réseau d'Affinité Vectorisé",
        "deep_learning_onnx" => "Screening Neural Haute Fréquence (Deep MLP ONNX)",
        "brics_fragment" => "Recombinaison Rétrosynthétique (Fragments BRICS)",
        "swarm_active_learning" => "Swarm Intelligence (Apprentissage Collaboratif)",
        _ => "IA Générative Autonome (Rust Native)",
    }
}

// SMIRKS de bioisostères et décorations médicinales
const SMIRKS_MUTATIONS: &[&str] = &[
    "[c:1][H]>>[c:1]F",
    "[c:1][H]>>[c:1]Cl",
    "[c:1][H]>>[c:1]Br",
    "[c:1][H]>>[c:1]OC",
    "[c:1][H]>>[c:1]C",
    "[c:1][H]>>[c:1]C#N",
    "[c:1][H]>>[c:1]C(F)(F)F",
    "[c:1][H]>>[c:1]NC(=O)C=C",
    "[c:1][H]>>[c:1]N1CCOCC1",
    "[c:1][H]>>[c:1]N1CCN(C)CC1",
    "[c:1][H]>>[c:1]N(C)C",
    "[c:1][H]>>[c:1]O",
    "[c:1][H]>>[c:1]C1CC1",
    "[c:1][H]>>[c:1]CC",
    "[c:1][H]>>[c:1]C#C",
    "[c:1][H]>>[c:1]C(=O)N",
    "[C:1][H]>>[C:1]F",
    "[C:1][H]>>[C:1]C",
    "[C:1][H]>>[C:1]O",
];

#[derive(Clone)]
pub struct CandidateMol {
    pub smiles: String,
    pub mol: Molecule,
    pub mw: f32,
    pub logp: f32,
    pub sascore: f32,
    pub pains_ok: bool,
    pub brenk_ok: bool,
}

#[derive(Clone)]
pub struct GeneratedBatch {
    pub batch_id: usize,
    pub candidates: Vec<CandidateMol>,
    pub gen_duration_secs: f64,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ConsensusTaskPayload {
    pub task_id: String,
    pub smiles: String,
    pub target: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConsensusResponsePayload {
    pub task_id: String,
    pub predicted_pkd: f32,
    pub docking_kcal: Option<f32>,
}

#[derive(Deserialize, Debug)]
struct IslandResponse {
    island_id: Option<String>,
    scaffold_name: Option<String>,
    scaffold_smarts: Option<String>,
    seeds: Option<Vec<String>>,
    migration_seeds: Option<Vec<String>>,
    consensus_task: Option<ConsensusTaskPayload>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MoleculeCandidatePayload {
    pub smiles: String,
    pub predicted_pkd: f32,
    pub mw: Option<f32>,
    pub logp: Option<f32>,
    pub docking_kcal: Option<f32>,
    pub sascore: Option<f32>,
    pub pains_ok: Option<bool>,
    pub brenk_ok: Option<bool>,
    pub scaffold: Option<String>,
    pub island_id: Option<String>,
    pub sdf_3d: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BatchSubmissionPayload {
    pub worker_id: String,
    pub evaluated_count: usize,
    pub hits: Vec<MoleculeCandidatePayload>,
    pub target: Option<String>,
    pub engine: Option<String>,
    pub island_id: Option<String>,
    pub public_key: Option<String>,
    pub signature: Option<String>,
    pub timestamp: Option<f64>,
    pub consensus_response: Option<ConsensusResponsePayload>,
}

#[derive(Deserialize, Debug)]
struct SubmitResponse {
    accepted_hits: Option<usize>,
    total_evaluated_globally: Option<usize>,
    leaderboard_rank: Option<usize>,
    worker_ip: Option<String>,
}

/// Identité cryptographique Ed25519 unique pour la signature de lots et l'audit consensuel
#[derive(Clone)]
pub struct VolunteerIdentity {
    pub signing_key: SigningKey,
    pub public_key_hex: String,
}

impl VolunteerIdentity {
    pub fn load_or_create(key_path: &Path) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let signing_key = if key_path.exists() {
            let bytes = std::fs::read(key_path)?;
            if bytes.len() == 32 {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                SigningKey::from_bytes(&arr)
            } else {
                let secret_bytes: [u8; 32] = rand::rng().random();
                let sk = SigningKey::from_bytes(&secret_bytes);
                let _ = std::fs::write(key_path, sk.to_bytes());
                sk
            }
        } else {
            let secret_bytes: [u8; 32] = rand::rng().random();
            let sk = SigningKey::from_bytes(&secret_bytes);
            let _ = std::fs::write(key_path, sk.to_bytes());
            sk
        };

        let verifying_key: VerifyingKey = signing_key.verifying_key();
        let pub_hex = hex::encode(verifying_key.to_bytes());
        Ok(Self {
            signing_key,
            public_key_hex: pub_hex,
        })
    }

    pub fn sign_message(&self, message: &[u8]) -> String {
        let signature = self.signing_key.sign(message);
        hex::encode(signature.to_bytes())
    }

    #[allow(dead_code)]
    pub fn verify_signature(public_key_hex: &str, message: &[u8], signature_hex: &str) -> bool {
        let Ok(pub_bytes) = hex::decode(public_key_hex) else { return false; };
        let Ok(sig_bytes) = hex::decode(signature_hex) else { return false; };
        if pub_bytes.len() != 32 || sig_bytes.len() != 64 {
            return false;
        }
        let mut p_arr = [0u8; 32];
        p_arr.copy_from_slice(&pub_bytes);
        let Ok(vk) = VerifyingKey::from_bytes(&p_arr) else { return false; };

        let mut s_arr = [0u8; 64];
        s_arr.copy_from_slice(&sig_bytes);
        let sig = ed25519_dalek::Signature::from_bytes(&s_arr);
        vk.verify_strict(message, &sig).is_ok()
    }
}

/// Initialise la session ONNX avec l'accélération matérielle native
/// (CoreML sur macOS Apple Silicon, DirectML sur Windows, CUDA sur Linux)
/// avec un repli automatique et transparent sur le CPU multi-threadé.
pub fn create_session(model_path: &str) -> Result<(Session, String), String> {
    // 1. Accélération matérielle Apple Silicon (NPU Neural Engine & GPU) sur macOS
    #[cfg(target_os = "macos")]
    {
        if let Ok(builder) = Session::builder() {
            let ep = ort::ep::CoreML::default()
                .with_compute_units(ort::ep::coreml::ComputeUnits::All)
                .build();
            if let Ok(mut builder) = builder.with_execution_providers([ep]) {
                if let Ok(session) = builder.commit_from_file(model_path) {
                    return Ok((
                        session,
                        "Apple Silicon NPU (Neural Engine) & GPU via CoreML".to_string(),
                    ));
                }
            }
        }
    }

    // 2. Accélération matérielle DirectML (GPU) sur Windows
    #[cfg(target_os = "windows")]
    {
        if let Ok(builder) = Session::builder() {
            let ep = ort::ep::DirectML::default().build();
            if let Ok(mut builder) = builder.with_execution_providers([ep]) {
                if let Ok(session) = builder.commit_from_file(model_path) {
                    return Ok((
                        session,
                        "DirectML (Accélération GPU Windows)".to_string(),
                    ));
                }
            }
        }
    }

    // 3. Accélération matérielle NVIDIA CUDA (GPU) sur Linux
    #[cfg(target_os = "linux")]
    {
        if let Ok(builder) = Session::builder() {
            let ep = ort::ep::CUDA::default().build();
            if let Ok(mut builder) = builder.with_execution_providers([ep]) {
                if let Ok(session) = builder.commit_from_file(model_path) {
                    return Ok((
                        session,
                        "NVIDIA CUDA (Accélération GPU Linux)".to_string(),
                    ));
                }
            }
        }
    }

    // 4. Repli robuste sur le CPU multi-threadé
    let mut builder = Session::builder().map_err(|e| format!("Erreur builder CPU: {:?}", e))?;
    let session = builder
        .commit_from_file(model_path)
        .map_err(|e| format!("Erreur chargement modèle CPU: {:?}", e))?;

    Ok((session, "CPU multi-threadé (Vectorisé)".to_string()))
}

/// Réservoir de squelettes hétérocycliques et fragments privilégiés en chimie médicinale
/// Utilisé pour le crossover afin d'explorer des familles de scaffolds inédites
pub const SCAFFOLD_RESERVOIR: &[&str] = &[
    "c1cc2c([nH]c1)cccc2",             // Indole
    "c1cc2c([nH]c1)cccn2",             // Azaindole
    "c1ncc2cc[nH]c2n1",                 // Pyrrolopyrimidine
    "c1ccc2[nH]cnc2c1",                 // Benzimidazole
    "c1ccc2[nH]ncc2c1",                 // Indazole
    "c1ccc2ncccc2c1",                   // Quinoline
    "c1ccc2cnccc2c1",                   // Isoquinoline
    "c1ccc(cc1)c2ccncc2",               // 4-Phénylpyridine
    "c1ccc(cc1)N2CCOCC2",               // Phénylmorpholine
    "c1ccc(cc1)N2CCN(C)CC2",            // Phényl-N-méthylpipérazine
    "Nc1ccc(F)c(Cl)c1",                 // 3-Chloro-4-fluoroaniline
    "Nc1cccc(C#C)c1",                   // 3-Éthynylaniline
    "Nc1ccc(OC)cc1",                    // 4-Méthoxyaniline
    "Nc1ccc(C(F)(F)F)cc1",              // 4-Trifluorométhylaniline
    "O=C1Nc2ccccc2C1",                  // Oxindole
    "c1ccc(cc1)C2CCOCC2",               // Phényltétrahydropyrane
    "c1ccc(cc1)C2CC2",                  // Cyclopropylbenzène
    "c1ccc(cc1)S(=O)(=O)N",             // Benzènesulfonamide
    "Nc1ncc(F)c(Nc2ccccc2)n1",          // Pyrimidinediamine
    "c1cc(ccn1)N2CCOCC2",               // Pyridinylmorpholine
    "c1cc2ncnc(Nc3ccccc3)c2cc1",        // Anilinoquinazoline
    "c1ccc(cc1)Nc2nccs2",               // Phénylaminothiazole
];

/// Vérifie si une liaison (u, v) fait partie d'un cycle (ring) par parcours BFS
fn is_ring_bond(mol: &Molecule, a: AtomIdx, b: AtomIdx) -> bool {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    visited.insert(a);
    for (nb, _) in mol.neighbors(a) {
        if nb != b {
            visited.insert(nb);
            queue.push_back(nb);
        }
    }
    while let Some(curr) = queue.pop_front() {
        if curr == b {
            return true;
        }
        for (nb, _) in mol.neighbors(curr) {
            if !visited.contains(&nb) {
                visited.insert(nb);
                queue.push_back(nb);
            }
        }
    }
    false
}

/// Extrait tous les indices d'atomes appartenant au composant connecté du côté de `start` sans traverser `excluded`
fn get_component_atoms(mol: &Molecule, start: AtomIdx, excluded: AtomIdx) -> HashSet<AtomIdx> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    visited.insert(start);
    queue.push_back(start);
    while let Some(curr) = queue.pop_front() {
        for (nb, _) in mol.neighbors(curr) {
            if nb != excluded && !visited.contains(&nb) {
                visited.insert(nb);
                queue.push_back(nb);
            }
        }
    }
    visited
}

/// Identifie toutes les liaisons simples hors-cycle éligibles au découpage rétrosynthétique / BRICS
pub fn find_crossover_bonds(mol: &Molecule) -> Vec<(AtomIdx, AtomIdx)> {
    // 1. D'abord les liaisons selon les règles BRICS
    let brics = chematic::chem::brics_bonds(mol);
    if !brics.is_empty() {
        return brics;
    }

    // 2. Repli rétrosynthétique : toute liaison simple acyclique entre atomes lourds non-terminaux
    let mut fallback = Vec::new();
    for bidx in 0..mol.bond_count() {
        let bond = mol.bond(BondIdx(bidx as u32));
        if !matches!(bond.order, BondOrder::Single | BondOrder::Up | BondOrder::Down) {
            continue;
        }
        let (a, b) = (bond.atom1, bond.atom2);
        let at_a = mol.atom(a);
        let at_b = mol.atom(b);
        if at_a.element.atomic_number() <= 1 || at_b.element.atomic_number() <= 1 {
            continue;
        }
        if mol.degree(a) < 2 || mol.degree(b) < 2 {
            continue;
        }
        if !is_ring_bond(mol, a, b) {
            fallback.push((a, b));
        }
    }
    fallback
}

/// Vérifie si la jonction entre deux éléments est favorable en chimie médicinale
fn is_chemically_favored_link(elem1: Element, elem2: Element) -> bool {
    match (elem1.atomic_number(), elem2.atomic_number()) {
        // C-C (alkyl, aryl, biaryl)
        (6, 6) => true,
        // C-N (amide, amine, urée, carbamate)
        (6, 7) | (7, 6) => true,
        // C-O (éther, ester)
        (6, 8) | (8, 6) => true,
        // C-S (thioéther)
        (6, 16) | (16, 6) => true,
        // S-N (sulfonamide)
        (16, 7) | (7, 16) => true,
        _ => false,
    }
}

/// Assemble deux fragments moléculaires (side1 de m1 et side2 de m2) en une nouvelle molécule complète
pub fn stitch_component_sides(
    m1: &Molecule,
    side1: &HashSet<AtomIdx>,
    a1: AtomIdx,
    m2: &Molecule,
    side2: &HashSet<AtomIdx>,
    a2: AtomIdx,
) -> Option<Molecule> {
    if side1.is_empty() || side2.is_empty() {
        return None;
    }

    let elem1 = m1.atom(a1).element;
    let elem2 = m2.atom(a2).element;
    if !is_chemically_favored_link(elem1, elem2) {
        return None;
    }

    let mut builder = MoleculeBuilder::new();
    let mut map1 = HashMap::with_capacity(side1.len());
    let mut map2 = HashMap::with_capacity(side2.len());

    // 1. Ajouter les atomes du côté 1
    for &idx in side1 {
        let at = m1.atom(idx);
        let new_idx = builder.add_atom(at.clone());
        map1.insert(idx, new_idx);
    }

    // 2. Ajouter les atomes du côté 2
    for &idx in side2 {
        let at = m2.atom(idx);
        let new_idx = builder.add_atom(at.clone());
        map2.insert(idx, new_idx);
    }

    // 3. Ajouter les liaisons internes du côté 1
    for bidx in 0..m1.bond_count() {
        let bond = m1.bond(BondIdx(bidx as u32));
        if side1.contains(&bond.atom1) && side1.contains(&bond.atom2) {
            let n1 = map1[&bond.atom1];
            let n2 = map1[&bond.atom2];
            let _ = builder.add_bond(n1, n2, bond.order);
        }
    }

    // 4. Ajouter les liaisons internes du côté 2
    for bidx in 0..m2.bond_count() {
        let bond = m2.bond(BondIdx(bidx as u32));
        if side2.contains(&bond.atom1) && side2.contains(&bond.atom2) {
            let n1 = map2[&bond.atom1];
            let n2 = map2[&bond.atom2];
            let _ = builder.add_bond(n1, n2, bond.order);
        }
    }

    // 5. Créer la nouvelle liaison simple entre les points d'attache a1 et a2
    let new_a1 = *map1.get(&a1)?;
    let new_a2 = *map2.get(&a2)?;
    builder.add_bond(new_a1, new_a2, BondOrder::Single).ok()?;

    Some(builder.build())
}

/// Effectue le croisement rétrosynthétique (BRICS crossover) entre deux molécules parentes
pub fn crossover_parents(m1: &Molecule, m2: &Molecule) -> Vec<Molecule> {
    let bonds1 = find_crossover_bonds(m1);
    let bonds2 = find_crossover_bonds(m2);

    if bonds1.is_empty() || bonds2.is_empty() {
        return Vec::new();
    }

    let mut children = Vec::new();

    for &(a1, b1) in &bonds1 {
        let side1_a = get_component_atoms(m1, a1, b1);
        let side1_b = get_component_atoms(m1, b1, a1);

        for &(a2, b2) in &bonds2 {
            let side2_a = get_component_atoms(m2, a2, b2);
            let side2_b = get_component_atoms(m2, b2, a2);

            // Recombinaisons (croisements) possibles
            if let Some(child) = stitch_component_sides(m1, &side1_a, a1, m2, &side2_b, b2) {
                children.push(child);
            }
            if let Some(child) = stitch_component_sides(m1, &side1_b, b1, m2, &side2_a, a2) {
                children.push(child);
            }
            if let Some(child) = stitch_component_sides(m1, &side1_a, a1, m2, &side2_a, a2) {
                children.push(child);
            }
            if let Some(child) = stitch_component_sides(m1, &side1_b, b1, m2, &side2_b, b2) {
                children.push(child);
            }

            if children.len() >= 6 {
                return children;
            }
        }
    }

    children
}

/// Génération De Novo de molécules avec Recombinaison Rétrosynthétique (BRICS Crossover)
/// et mutations bioisostères SMIRKS, avec filtres ADMET, SAscore et PAINS intégrés (100% Rust)
pub fn generate_molecules(pool_smiles: &[String], count: usize) -> Vec<CandidateMol> {
    let mut candidates = Vec::with_capacity(count);
    let mut seen_smiles = HashSet::new();
    let mut rng = rand::rng();

    // Insérer les graines dans le set pour éviter de régénérer l'identique
    for s in pool_smiles {
        seen_smiles.insert(s.clone());
    }

    let max_attempts = count * 20;
    let mut attempts = 0;

    // Parser les molécules du pool initial
    let mut parsed_pool: Vec<Molecule> = pool_smiles
        .iter()
        .filter_map(|s| parse(s).ok())
        .collect();

    // 1. Phase A : Recombinaison Rétrosynthétique / BRICS Crossover
    // Génère des squelettes inédits par hybridation de fragments et exploration de scaffolds variés
    while candidates.len() < count && attempts < max_attempts {
        attempts += 1;

        if parsed_pool.is_empty() {
            break;
        }

        // Tirer deux parents pour le croisement :
        // - Soit deux molécules du pool (croisement génétique direct)
        // - Soit une molécule du pool et un scaffold/fragment du réservoir diversifié
        let (p1, p2) = if parsed_pool.len() >= 2 && rng.random_bool(0.4) {
            let idx1 = rng.random_range(0..parsed_pool.len());
            let mut idx2 = rng.random_range(0..parsed_pool.len());
            if idx1 == idx2 {
                idx2 = (idx1 + 1) % parsed_pool.len();
            }
            (parsed_pool[idx1].clone(), parsed_pool[idx2].clone())
        } else {
            let p1 = match parsed_pool.choose(&mut rng) {
                Some(m) => m.clone(),
                None => break,
            };
            let scaffold_smi = SCAFFOLD_RESERVOIR.choose(&mut rng).unwrap();
            let p2 = match parse(scaffold_smi) {
                Ok(m) => m,
                Err(_) => continue,
            };
            (p1, p2)
        };

        // Exécuter le croisement rétrosynthétique
        let recombinant_children = crossover_parents(&p1, &p2);

        for child in recombinant_children {
            let can_smi = canonical_smiles(&child);
            if seen_smiles.contains(&can_smi) {
                continue;
            }

            // Calcul des propriétés ADMET & Drug-likeness
            let mw = chematic::chem::molecular_weight(&child) as f32;
            if mw < 160.0 || mw > 700.0 {
                continue;
            }

            let logp = chematic::chem::logp_crippen(&child) as f32;
            if logp < -2.0 || logp > 6.0 {
                continue;
            }

            let sascore = chematic::chem::sa_score(&child) as f32;
            if sascore > 5.5 {
                continue;
            }

            let pains_ok = chematic::chem::alerts::pains_passes(&child);
            if !pains_ok {
                continue;
            }

            let brenk_ok = chematic::chem::alerts::brenk_passes(&child);

            seen_smiles.insert(can_smi.clone());
            parsed_pool.push(child.clone()); // Enrichit le pool génétique pour les générations suivantes
            candidates.push(CandidateMol {
                smiles: can_smi,
                mol: child,
                mw: (mw * 10.0).round() / 10.0,
                logp: (logp * 100.0).round() / 100.0,
                sascore: (sascore * 100.0).round() / 100.0,
                pains_ok,
                brenk_ok,
            });

            if candidates.len() >= count {
                break;
            }
        }
    }

    // 2. Phase B : Mutations et décorations bioisostères (SMIRKS)
    // Permet d'affiner les groupements périphériques et de compléter le lot jusqu'au count cible
    let mut mutation_attempts = 0;
    while candidates.len() < count && mutation_attempts < count * 15 {
        mutation_attempts += 1;

        let seed_mol = if !candidates.is_empty() && rng.random_bool(0.6) {
            candidates.choose(&mut rng).map(|c| &c.mol)
        } else {
            parsed_pool.choose(&mut rng)
        };

        let mol = match seed_mol {
            Some(m) => m,
            None => break,
        };

        let mol_h = chematic::chem::add_hydrogens(mol);
        let smirks = match SMIRKS_MUTATIONS.choose(&mut rng) {
            Some(&s) => s,
            None => continue,
        };

        if let Ok(product_groups) = chematic::rxn::run_reactants(smirks, &[&mol_h]) {
            for group in product_groups {
                for prod in group {
                    let prod_no_h = chematic::chem::remove_hydrogens(&prod);
                    let can_smi = canonical_smiles(&prod_no_h);

                    if seen_smiles.contains(&can_smi) {
                        continue;
                    }

                    let mw = chematic::chem::molecular_weight(&prod_no_h) as f32;
                    if mw < 160.0 || mw > 700.0 {
                        continue;
                    }

                    let logp = chematic::chem::logp_crippen(&prod_no_h) as f32;
                    if logp < -2.0 || logp > 6.0 {
                        continue;
                    }

                    let sascore = chematic::chem::sa_score(&prod_no_h) as f32;
                    if sascore > 5.5 {
                        continue;
                    }

                    let pains_ok = chematic::chem::alerts::pains_passes(&prod_no_h);
                    if !pains_ok {
                        continue;
                    }

                    let brenk_ok = chematic::chem::alerts::brenk_passes(&prod_no_h);

                    seen_smiles.insert(can_smi.clone());
                    candidates.push(CandidateMol {
                        smiles: can_smi,
                        mol: prod_no_h,
                        mw: (mw * 10.0).round() / 10.0,
                        logp: (logp * 100.0).round() / 100.0,
                        sascore: (sascore * 100.0).round() / 100.0,
                        pains_ok,
                        brenk_ok,
                    });

                    if candidates.len() >= count {
                        break;
                    }
                }
                if candidates.len() >= count {
                    break;
                }
            }
        }
    }

    candidates
}

/// Inférence ONNX vectorisée par batch complet (K, 2048) en UN SEUL APPEL
pub fn evaluate_candidates(
    session: &mut Session,
    candidates: &[CandidateMol],
) -> Result<Vec<f32>, String> {
    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    let k = candidates.len();
    let mut fp_matrix = vec![0.0f32; k * 2048];

    for (row, cand) in candidates.iter().enumerate() {
        let fp = ecfp4_rdkit_invariants(&cand.mol);
        let offset = row * 2048;
        for bit in 0..2048 {
            if fp.get(bit) {
                fp_matrix[offset + bit] = 1.0;
            }
        }
    }

    let input_array = Array2::from_shape_vec((k, 2048), fp_matrix)
        .map_err(|e| format!("Erreur construction matrice ndarray: {:?}", e))?;

    let tensor = ort::value::Tensor::from_array(input_array)
        .map_err(|e| format!("Erreur création Tensor ORT: {:?}", e))?;

    let outputs = session
        .run(ort::inputs!["fingerprint" => tensor])
        .map_err(|e| format!("Erreur inférence ORT: {:?}", e))?;

    let affinity_tensor = outputs["affinity"]
        .try_extract_tensor::<f32>()
        .map_err(|e| format!("Erreur extraction affinité: {:?}", e))?;

    let raw_scores = affinity_tensor.1;
    let scores: Vec<f32> = raw_scores
        .iter()
        .map(|&s| (s * 100.0).round() / 100.0)
        .collect();

    Ok(scores)
}

#[derive(Clone, Debug)]
pub struct DockingResult {
    pub affinity_kcal: f32,
    #[allow(dead_code)]
    pub is_favorable: bool,
}

/// Localise le binaire AutoDock Vina (sidecar packagé ou binaire local multiplateforme)
pub fn find_vina_binary() -> Option<PathBuf> {
    // 1. Variable d'environnement prioritaire
    if let Ok(custom_path) = std::env::var("VINA_PATH") {
        let p = PathBuf::from(custom_path);
        if p.exists() {
            return Some(p);
        }
    }

    // 2. Noms de fichiers candidats selon la plateforme
    let mut names = vec!["vina", "vina.exe"];

    #[cfg(target_os = "windows")]
    {
        names.extend_from_slice(&[
            "vina-x86_64-pc-windows-msvc.exe",
            "vina.exe",
        ]);
    }

    #[cfg(target_os = "macos")]
    {
        names.extend_from_slice(&[
            "vina-aarch64-apple-darwin",
            "vina-x86_64-apple-darwin",
            "vina",
        ]);
    }

    #[cfg(target_os = "linux")]
    {
        names.extend_from_slice(&[
            "vina-x86_64-unknown-linux-gnu",
            "vina-aarch64-unknown-linux-gnu",
            "vina",
        ]);
    }

    // 3. Chemins relatifs de recherche en développement
    let search_dirs = [
        PathBuf::from("bin"),
        PathBuf::from("helixgrid-ui/bin"),
        PathBuf::from("helixgrid-ui/src-tauri/bin"),
        PathBuf::from("../bin"),
        PathBuf::from("../src-tauri/bin"),
        PathBuf::from("../../bin"),
        PathBuf::from("."),
    ];

    for dir in &search_dirs {
        for name in &names {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    // 4. Chemins relatifs à l'exécutable en production (bundle Tauri)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let bundle_dirs = [
                parent.to_path_buf(),
                parent.join("bin"),
                parent.join("../Resources"),
                parent.join("../Resources/bin"),
            ];

            for dir in &bundle_dirs {
                for name in &names {
                    let candidate = dir.join(name);
                    if candidate.exists() {
                        return Some(candidate);
                    }
                }
            }
        }
    }

    // 5. Recherche dans le PATH système
    if let Ok(path_var) = std::env::var("PATH") {
        let separator = if cfg!(target_os = "windows") { ';' } else { ':' };
        for dir in path_var.split(separator) {
            for name in &names {
                let candidate = PathBuf::from(dir).join(name);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }

    None
}

/// Dossier local de cache persistant pour les modèles et récepteurs téléchargés à la demande
pub fn get_cache_dir() -> PathBuf {
    std::env::temp_dir().join("helixgrid_cache")
}

/// Télécharge un fichier de manière asynchrone et atomique (écriture temporaire puis renommage)
async fn download_file(url: &str, dest: &Path) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Erreur réseau pour {}: {}", url, e))?;

    if !response.status().is_success() {
        return Err(format!("Échec HTTP {} pour {}", response.status(), url));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Erreur lecture données {}: {}", url, e))?;

    let temp_dest = dest.with_extension("tmp_download");
    std::fs::write(&temp_dest, &bytes)
        .map_err(|e| format!("Erreur écriture fichier temporaire: {}", e))?;
    std::fs::rename(&temp_dest, dest)
        .map_err(|e| format!("Erreur renommage fichier final: {}", e))?;

    Ok(())
}

/// Localise un modèle ONNX (et vérifie la présence du fichier de données .data associé si nécessaire)
pub fn find_model_file(model_filename: &str) -> Option<PathBuf> {
    let data_filename = format!("{}.data", model_filename);
    let cache_dir = get_cache_dir().join("models");

    let candidates = [
        PathBuf::from(model_filename),
        PathBuf::from("helixgrid-ui/src-tauri").join(model_filename),
        PathBuf::from("helixgrid-ui").join(model_filename),
        PathBuf::from("../").join(model_filename),
        PathBuf::from("../../").join(model_filename),
        cache_dir.join(model_filename),
    ];

    for c in &candidates {
        if c.exists() {
            let companion = c.with_file_name(&data_filename);
            if !companion.exists() || companion.is_file() {
                return Some(c.clone());
            }
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let exe_candidates = [
                parent.join(model_filename),
                parent.join("../Resources").join(model_filename),
                parent.join("../Resources/models").join(model_filename),
                parent.join("models").join(model_filename),
            ];
            for c in &exe_candidates {
                if c.exists() {
                    return Some(c.clone());
                }
            }
        }
    }

    None
}

/// Localise le fichier PDBQT du récepteur protéique de la cible (bundle, dev, ou cache téléchargé)
pub fn find_receptor_file(pdb_id: &str) -> Option<PathBuf> {
    let filename = format!("{}.pdbqt", pdb_id);
    let cache_dir = get_cache_dir().join("receptors");

    let candidates = [
        PathBuf::from("data/receptors").join(&filename),
        PathBuf::from("helixgrid-ui/data/receptors").join(&filename),
        PathBuf::from("helixgrid-ui/src-tauri/data/receptors").join(&filename),
        PathBuf::from("../data/receptors").join(&filename),
        PathBuf::from("../../data/receptors").join(&filename),
        cache_dir.join(&filename),
    ];

    for c in &candidates {
        if c.exists() {
            return Some(c.clone());
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let exe_candidates = [
                parent.join("../Resources/data/receptors").join(&filename),
                parent.join("../Resources").join(&filename),
                parent.join("data/receptors").join(&filename),
                parent.join(&filename),
            ];
            for c in &exe_candidates {
                if c.exists() {
                    return Some(c.clone());
                }
            }
        }
    }

    None
}

/// Garantit que le modèle ONNX et le récepteur 3D de la cible sont prêts :
/// vérifie le bundle local en priorité, sinon les télécharge à la demande au premier démarrage.
pub async fn ensure_model_and_receptor(
    window: &Window,
    target_info: &TargetConfig,
) -> Result<PathBuf, String> {
    // 1. Modèle ONNX
    let model_path = if let Some(p) = find_model_file(target_info.model_file) {
        let _ = window.emit(
            "worker-log",
            format!("  [Modèle ONNX] '{}' prêt (détecté dans le bundle/local)", target_info.model_file),
        );
        p
    } else {
        let _ = window.emit(
            "worker-log",
            format!(
                "  [Réseau] Modèle spécialisé '{}' introuvable dans le bundle. Téléchargement à la demande...",
                target_info.model_file
            ),
        );
        let dest = get_cache_dir().join("models").join(target_info.model_file);
        let dest_data = get_cache_dir().join("models").join(format!("{}.data", target_info.model_file));

        let model_urls = [
            format!("https://raw.githubusercontent.com/CamilGrondin/helixgrid-updates/main/{}", target_info.model_file),
            format!("https://github.com/CamilGrondin/helixgrid-updates/releases/download/v0.3.0/{}", target_info.model_file),
            format!("https://raw.githubusercontent.com/CamilGrondin/HelixGrid/main/{}", target_info.model_file),
        ];

        let mut downloaded = false;
        for url in &model_urls {
            if let Ok(()) = download_file(url, &dest).await {
                let data_url = format!("{}.data", url);
                let _ = download_file(&data_url, &dest_data).await;
                downloaded = true;
                break;
            }
        }

        if downloaded && dest.exists() {
            let _ = window.emit(
                "worker-log",
                format!("  [✓] Modèle '{}' téléchargé et mis en cache avec succès.", target_info.model_file),
            );
            dest
        } else {
            return Err(format!("Impossible de trouver ou télécharger le modèle '{}'", target_info.model_file));
        }
    };

    // 2. Fichier Récepteur 3D (.pdbqt)
    if let Some(p) = find_receptor_file(target_info.pdb) {
        let _ = window.emit(
            "worker-log",
            format!("  [Récepteur 3D] '{}.pdbqt' prêt ({})", target_info.pdb, p.display()),
        );
    } else {
        let _ = window.emit(
            "worker-log",
            format!(
                "  [Réseau] Récepteur 3D '{}.pdbqt' introuvable dans le bundle. Téléchargement à la demande...",
                target_info.pdb
            ),
        );
        let dest = get_cache_dir().join("receptors").join(format!("{}.pdbqt", target_info.pdb));
        let receptor_urls = [
            format!("https://raw.githubusercontent.com/CamilGrondin/helixgrid-updates/main/data/receptors/{}.pdbqt", target_info.pdb),
            format!("https://raw.githubusercontent.com/CamilGrondin/HelixGrid/main/data/receptors/{}.pdbqt", target_info.pdb),
            format!("https://github.com/CamilGrondin/helixgrid-updates/releases/download/v0.3.0/{}.pdbqt", target_info.pdb),
        ];

        let mut downloaded = false;
        for url in &receptor_urls {
            if let Ok(()) = download_file(url, &dest).await {
                downloaded = true;
                break;
            }
        }

        if downloaded && dest.exists() {
            let _ = window.emit(
                "worker-log",
                format!("  [✓] Récepteur '{}.pdbqt' téléchargé et mis en cache avec succès.", target_info.pdb),
            );
        } else {
            let _ = window.emit(
                "worker-log",
                format!("  [!] Récepteur '{}.pdbqt' indisponible (docking Vina désactivé pour cette cible)", target_info.pdb),
            );
        }
    }

    Ok(model_path)
}

/// Génère les coordonnées 3D optimisées (ETKDG + MMFF94) et produit le format PDBQT conforme AutoDock Vina
pub fn molecule_to_pdbqt(mol: &Molecule) -> Option<String> {
    let mol_h = chematic::chem::add_hydrogens(mol);
    let coords = chematic::threed::generate_coords_etkdg(&mol_h);
    let opt_coords = chematic::threed::minimize_mmff94(&mol_h, coords);

    let mut ligand_pdbqt = String::with_capacity(mol_h.atom_count() * 85 + 32);
    ligand_pdbqt.push_str("ROOT\n");

    for (i, (idx, atom)) in mol_h.atoms().enumerate() {
        let pt = opt_coords.get(idx);
        let sym = atom.element.symbol();
        let ad_type = match sym {
            "C" => {
                if atom.aromatic {
                    "A"
                } else {
                    "C"
                }
            }
            "N" => {
                if atom.aromatic {
                    "NA"
                } else {
                    "N"
                }
            }
            "O" => "OA",
            "S" => {
                if atom.aromatic {
                    "SA"
                } else {
                    "S"
                }
            }
            "P" => "P",
            "F" => "F",
            "Cl" => "Cl",
            "Br" => "Br",
            "I" => "I",
            "H" => {
                let is_polar = mol_h.neighbors(idx).any(|(n_idx, _)| {
                    matches!(mol_h.atom(n_idx).element.symbol(), "N" | "O" | "S")
                });
                if is_polar {
                    "HD"
                } else {
                    "H"
                }
            }
            _ => sym,
        };

        let atom_name_raw = format!("{}{}", sym, i + 1);
        let atom_name: String = atom_name_raw.chars().take(4).collect();

        // Standard PDBQT line formatting avec colonnes PDB strictes
        // Cols: 1-6 ATOM, 7-11 serial, 13-16 name, 18-20 resName, 22 chain, 23-26 resSeq
        // 31-38 x, 39-46 y, 47-54 z, 55-60 occ, 61-66 temp, 71-76 charge, 78-79 type
        let line = format!(
            "ATOM  {:5} {:<4} LIG A   1    {:8.3}{:8.3}{:8.3}  1.00  0.00    {:+6.3} {:<2}\n",
            i + 1,
            atom_name,
            pt.x,
            pt.y,
            pt.z,
            0.0,
            ad_type
        );
        ligand_pdbqt.push_str(&line);
    }

    ligand_pdbqt.push_str("ENDROOT\nTORSDOF 0\n");
    Some(ligand_pdbqt)
}

/// Génère le bloc MolBlock/SDF 3D conforme au standard MDL V2000 avec coordonnées ETKDG minimisées
pub fn molecule_to_sdf_3d(mol: &Molecule, title: &str) -> Option<String> {
    let mol_h = chematic::chem::add_hydrogens(mol);
    let coords = chematic::threed::generate_coords_etkdg(&mol_h);
    let opt_coords = chematic::threed::minimize_mmff94(&mol_h, coords);

    let metadata = chematic::mol::MolMetadata {
        name: title.to_string(),
        comment: "HelixGrid Native 3D ETKDG+MMFF94".to_string(),
        v3000_sgroups: Vec::new(),
        v3000_bond_properties: Vec::new(),
        v3000_atom_properties: Vec::new(),
    };

    Some(chematic::mol::write_mol_with_conformer(&mol_h, &metadata, &opt_coords))
}

/// Exécute l'amarrage moléculaire (docking) structure-based via le sidecar AutoDock Vina
pub fn run_docking(
    mol: &Molecule,
    target_config: &TargetConfig,
    exhaustiveness: usize,
) -> Option<DockingResult> {
    let vina_bin = find_vina_binary()?;
    let receptor_path = find_receptor_file(target_config.pdb)?;

    let ligand_pdbqt = molecule_to_pdbqt(mol)?;

    let temp_dir = std::env::temp_dir();
    let temp_id = uuid::Uuid::new_v4().as_simple().to_string();
    let ligand_path = temp_dir.join(format!("lig_{}.pdbqt", temp_id));
    let out_path = temp_dir.join(format!("lig_{}_out.pdbqt", temp_id));

    if std::fs::write(&ligand_path, &ligand_pdbqt).is_err() {
        return None;
    }

    let cx = target_config.center.0.to_string();
    let cy = target_config.center.1.to_string();
    let cz = target_config.center.2.to_string();
    let sx = target_config.size.0.to_string();
    let sy = target_config.size.1.to_string();
    let sz = target_config.size.2.to_string();
    let exh = exhaustiveness.to_string();

    let output = std::process::Command::new(&vina_bin)
        .arg("--receptor").arg(&receptor_path)
        .arg("--ligand").arg(&ligand_path)
        .arg("--out").arg(&out_path)
        .arg("--center_x").arg(&cx)
        .arg("--center_y").arg(&cy)
        .arg("--center_z").arg(&cz)
        .arg("--size_x").arg(&sx)
        .arg("--size_y").arg(&sy)
        .arg("--size_z").arg(&sz)
        .arg("--exhaustiveness").arg(&exh)
        .arg("--num_modes").arg("1")
        .arg("--verbosity").arg("0")
        .output();

    let mut affinity_kcal = None;

    if out_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&out_path) {
            for line in content.lines() {
                if line.starts_with("REMARK VINA RESULT:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 4 {
                        if let Ok(val) = parts[3].parse::<f32>() {
                            affinity_kcal = Some(val);
                            break;
                        }
                    }
                }
            }
        }
    }

    if affinity_kcal.is_none() {
        if let Ok(ref out) = output {
            let stdout_str = String::from_utf8_lossy(&out.stdout);
            for line in stdout_str.lines() {
                let trimmed = line.trim();
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 && parts[0] == "1" {
                    if let Ok(val) = parts[1].parse::<f32>() {
                        affinity_kcal = Some(val);
                        break;
                    }
                }
            }
        }
    }

    let _ = std::fs::remove_file(&ligand_path);
    let _ = std::fs::remove_file(&out_path);

    affinity_kcal.map(|aff| {
        let rounded = (aff * 100.0).round() / 100.0;
        DockingResult {
            affinity_kcal: rounded,
            is_favorable: rounded <= -6.0,
        }
    })
}

/// Initialise la base de données tampon SQLite locale en mode WAL
pub fn init_sqlite_buffer(db_path: &Path) -> Result<(), rusqlite::Error> {
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = rusqlite::Connection::open(db_path)?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA busy_timeout = 5000;
         CREATE TABLE IF NOT EXISTS pending_submissions (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             payload_json TEXT NOT NULL,
             target TEXT,
             hits_count INTEGER NOT NULL,
             evaluated_count INTEGER NOT NULL,
             created_at REAL NOT NULL,
             retry_count INTEGER DEFAULT 0
         );",
    )?;
    Ok(())
}

/// Sauvegarde un lot dans la base SQLite locale tampon si le réseau est indisponible
pub fn save_pending_submission(
    db_path: &Path,
    payload: &BatchSubmissionPayload,
) -> Result<i64, rusqlite::Error> {
    let conn = rusqlite::Connection::open(db_path)?;
    let json_str = serde_json::to_string(payload).unwrap_or_default();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();
    conn.execute(
        "INSERT INTO pending_submissions (payload_json, target, hits_count, evaluated_count, created_at, retry_count)
         VALUES (?1, ?2, ?3, ?4, ?5, 0)",
        params![
            json_str,
            payload.target,
            payload.hits.len() as i64,
            payload.evaluated_count as i64,
            now
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Récupère les lots en attente dans la base SQLite locale tampon
pub fn get_pending_submissions(
    db_path: &Path,
    limit: usize,
) -> Result<Vec<(i64, BatchSubmissionPayload)>, rusqlite::Error> {
    let conn = rusqlite::Connection::open(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT id, payload_json FROM pending_submissions ORDER BY id ASC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit as i64], |row| {
        let id: i64 = row.get(0)?;
        let json_str: String = row.get(1)?;
        Ok((id, json_str))
    })?;

    let mut result = Vec::new();
    for r in rows {
        if let Ok((id, json_str)) = r {
            if let Ok(payload) = serde_json::from_str::<BatchSubmissionPayload>(&json_str) {
                result.push((id, payload));
            } else {
                let _ = delete_pending_submission(db_path, id);
            }
        }
    }
    Ok(result)
}

/// Supprime un lot persisté une fois transmis avec succès
pub fn delete_pending_submission(db_path: &Path, id: i64) -> Result<(), rusqlite::Error> {
    let conn = rusqlite::Connection::open(db_path)?;
    conn.execute("DELETE FROM pending_submissions WHERE id = ?1", [id])?;
    Ok(())
}

/// Compte le nombre de lots en attente dans le tampon SQLite local
pub fn count_pending_submissions(db_path: &Path) -> Result<usize, rusqlite::Error> {
    let conn = rusqlite::Connection::open(db_path)?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pending_submissions",
        [],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}

/// Expédie par paquets les lots accumulés dans la base SQLite locale tampon
pub async fn flush_sqlite_buffer(
    client: &Client,
    window: &Window,
    db_path: &Path,
    submit_url: &str,
) {
    let pending_items = match get_pending_submissions(db_path, 25) {
        Ok(items) => items,
        Err(_) => return,
    };

    if pending_items.is_empty() {
        return;
    }

    let mut flushed_batches = 0usize;
    let mut total_hits_flushed = 0usize;
    let mut total_screened_flushed = 0usize;

    for (id, payload) in pending_items {
        let hits_in_batch = payload.hits.len();
        let screened_in_batch = payload.evaluated_count;
        match client.post(submit_url).json(&payload).send().await {
            Ok(resp) if resp.status().is_success() => {
                let _ = delete_pending_submission(db_path, id);
                flushed_batches += 1;
                total_hits_flushed += hits_in_batch;
                total_screened_flushed += screened_in_batch;
            }
            _ => {
                // Le réseau est indisponible, suspendre la vidange
                break;
            }
        }
    }

    if flushed_batches > 0 {
        let remaining = count_pending_submissions(db_path).unwrap_or(0);
        let _ = window.emit(
            "worker-log",
            format!(
                "[✓] Vidange du tampon SQLite : {} lot(s) ({} criblées, +{} pépites) expédiés avec succès au serveur ! (Restants : {})",
                flushed_batches, total_screened_flushed, total_hits_flushed, remaining
            ),
        );
    }
}

/// Tâche d'arrière-plan Tokio indépendante pour l'expédition non bloquante sur le réseau
/// et la persistance SQLite tampon en cas de coupure réseau
pub async fn run_background_network_shipper(
    client: Client,
    window: Window,
    db_path: PathBuf,
    mut submission_rx: tokio::sync::mpsc::Receiver<BatchSubmissionPayload>,
) {
    let submit_url = format!("{}/hits/submit", get_server_url());
    let mut retry_ticker = tokio::time::interval(Duration::from_secs(8));
    retry_ticker.tick().await;

    loop {
        tokio::select! {
            maybe_sub = submission_rx.recv() => {
                match maybe_sub {
                    Some(payload) => {
                        let hits_count = payload.hits.len();
                        let screened = payload.evaluated_count;

                        // Expédition réseau non bloquante pour la boucle de calcul
                        match client.post(&submit_url).json(&payload).send().await {
                            Ok(resp) if resp.status().is_success() => {
                                if let Ok(data) = resp.json::<SubmitResponse>().await {
                                    let accepted = data.accepted_hits.unwrap_or(0);
                                    let global_total = data.total_evaluated_globally.unwrap_or(0);
                                    let rank_str = data
                                        .leaderboard_rank
                                        .map(|r| format!("| Rang mondial : #{}", r))
                                        .unwrap_or_default();
                                    let ip_str = data
                                        .worker_ip
                                        .map(|ip| format!("| IP : {}", ip))
                                        .unwrap_or_default();

                                    let _ = window.emit(
                                        "worker-log",
                                        format!(
                                            "[✓] Résultats synchronisés (+{} pépites {} {})",
                                            accepted, rank_str, ip_str
                                        ),
                                    );
                                    let _ = window.emit(
                                        "worker-log",
                                        format!(
                                            "    Criblées : {} | Total mondial persisté : {}",
                                            screened, global_total
                                        ),
                                    );
                                }

                                // Vidanger le tampon SQLite local si des éléments étaient restés en attente
                                flush_sqlite_buffer(&client, &window, &db_path, &submit_url).await;
                            }
                            _ => {
                                // Réseau indisponible ou serveur hors-ligne -> Sauvegarde dans SQLite
                                match save_pending_submission(&db_path, &payload) {
                                    Ok(_) => {
                                        let pending_total = count_pending_submissions(&db_path).unwrap_or(1);
                                        let _ = window.emit(
                                            "worker-log",
                                            format!(
                                                "[~] Serveur hors-ligne : lot ({} criblées, {} pépites) persisté dans le tampon SQLite local (En attente : {}). Calcul autonome ininterrompu.",
                                                screened, hits_count, pending_total
                                            ),
                                        );
                                    }
                                    Err(e) => {
                                        let _ = window.emit(
                                            "worker-log",
                                            format!("[!] Erreur persistance tampon SQLite : {}", e),
                                        );
                                    }
                                }
                            }
                        }
                    }
                    None => {
                        // Canal fermé (arrêt du worker)
                        flush_sqlite_buffer(&client, &window, &db_path, &submit_url).await;
                        break;
                    }
                }
            }

            // Tentative périodique de vidange du tampon dès retour du réseau
            _ = retry_ticker.tick() => {
                if count_pending_submissions(&db_path).unwrap_or(0) > 0 {
                    flush_sqlite_buffer(&client, &window, &db_path, &submit_url).await;
                }
            }
        }
    }
}

/// Boucle du thread producteur : génère les molécules du lot N+1 pendant que le lot N est évalué
pub fn producer_loop(
    tx: tokio::sync::mpsc::Sender<GeneratedBatch>,
    shared_seeds: Arc<std::sync::RwLock<Vec<String>>>,
    shared_migration_seeds: Arc<std::sync::RwLock<Vec<String>>>,
    cpu_limit: Arc<AtomicU32>,
    idle_only: Arc<AtomicBool>,
    is_idle: Arc<AtomicBool>,
    stop_signal: Arc<AtomicBool>,
) {
    let mut batch_counter = 0usize;

    while !stop_signal.load(Ordering::Relaxed) {
        // Contrôle du mode économiseur d'écran (inactif uniquement)
        if idle_only.load(Ordering::Relaxed) && !is_idle.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(500));
            continue;
        }

        // Récupération des graines actuelles
        let mut pool = match shared_seeds.read() {
            Ok(guard) => guard.clone(),
            Err(_) => break,
        };

        if let Ok(mig_guard) = shared_migration_seeds.read() {
            if !mig_guard.is_empty() {
                let mut rng = rand::rng();
                if rng.random_bool(0.15) {
                    if let Some(s) = mig_guard.choose(&mut rng) {
                        pool.push(s.clone());
                    }
                }
            }
        }

        let current_cpu_pct = cpu_limit.load(Ordering::Relaxed).clamp(10, 100);
        let effective_batch_size = if current_cpu_pct <= 25 {
            BATCH_SIZE / 2
        } else {
            BATCH_SIZE
        };

        let start_gen = Instant::now();
        let candidates = generate_molecules(&pool, effective_batch_size);
        let gen_duration_secs = start_gen.elapsed().as_secs_f64();

        if candidates.is_empty() {
            std::thread::sleep(Duration::from_millis(800));
            continue;
        }

        batch_counter += 1;
        let batch = GeneratedBatch {
            batch_id: batch_counter,
            candidates,
            gen_duration_secs,
        };

        // Envoi bloquant dans la file d'attente Producteur-Consommateur (bounded channel de capacité 2).
        // Si le lot N est en cours d'évaluation et le lot N+1 attend déjà, ce thread s'endort automatiquement
        // sans consommation CPU inutile jusqu'à ce que le consommateur dépile le lot N.
        if tx.blocking_send(batch).is_err() {
            break;
        }

        // Modulation duty-cycle CPU uniquement si l'utilisateur demande une réduction (< 100%)
        // Si CPU à 100% : 0 ms de pause pour saturer à plein régime les cœurs de calcul
        if current_cpu_pct < 100 {
            let sleep_ms = ((gen_duration_secs * 1000.0) as u64)
                * (100 - current_cpu_pct as u64)
                / (current_cpu_pct as u64);
            if sleep_ms > 0 {
                std::thread::sleep(Duration::from_millis(sleep_ms));
            }
        }
    }
}

/// Résout le répertoire persistant d'application pour les clés Ed25519 et le tampon SQLite.
/// Utilise le dossier de données de l'application (Tauri app_data_dir), avec fallbacks résilients.
pub fn resolve_app_storage_dir(window: &Window) -> PathBuf {
    if let Ok(dir) = window.app_handle().path().app_data_dir() {
        if std::fs::create_dir_all(&dir).is_ok() {
            return dir;
        }
    }
    if let Ok(dir) = window.app_handle().path().app_local_data_dir() {
        if std::fs::create_dir_all(&dir).is_ok() {
            return dir;
        }
    }
    if let Ok(curr) = std::env::current_dir() {
        if curr != Path::new("/") && curr.exists() {
            let test_file = curr.join(".helixgrid_write_test");
            if std::fs::write(&test_file, b"ok").is_ok() {
                let _ = std::fs::remove_file(test_file);
                return curr;
            }
        }
    }
    let tmp = std::env::temp_dir().join("helixgrid");
    let _ = std::fs::create_dir_all(&tmp);
    tmp
}

/// Boucle principale du worker 100% natif en Rust avec pipeline Producteur-Consommateur
pub async fn run_worker_loop(
    window: Window,
    model_path: String,
    target: String,
    engine: String,
    cpu_limit: Arc<AtomicU32>,
    idle_only: Arc<AtomicBool>,
    is_idle: Arc<AtomicBool>,
    volunteer_pseudo: Arc<tokio::sync::RwLock<Option<String>>>,
    stop_signal: Arc<AtomicBool>,
    is_running: Arc<AtomicBool>,
) {
    let initial_pseudo = volunteer_pseudo.read().await.clone();
    let worker_id = match initial_pseudo {
        Some(p) if !p.trim().is_empty() => {
            let sanitized = p.trim().replace(' ', "_");
            sanitized.chars().take(28).collect::<String>()
        }
        _ => format!(
            "volunteer_{}",
            uuid::Uuid::new_v4()
                .as_simple()
                .to_string()
                .chars()
                .take(6)
                .collect::<String>()
        ),
    };

    let target_info = get_target_config(&target);
    let engine_name = get_engine_name(&engine);
    let server_url = get_server_url();

    let mut seeds: Vec<String> = target_info.seeds.iter().map(|s| s.to_string()).collect();
    let mut migration_seeds: Vec<String> = Vec::new();
    let mut current_island_id: Option<String> = None;
    let mut current_scaffold_name: Option<String> = None;

    let _ = window.emit("worker-log", "=".repeat(75));
    let _ = window.emit(
        "worker-log",
        "  HELIXGRID - WORKER BÉNÉVOLE 100% NATIF (RUST)".to_string(),
    );
    let _ = window.emit("worker-log", format!("  Worker ID   : {}", worker_id));

    // Gestion de l'identité cryptographique Ed25519 du bénévole dans le stockage utilisateur
    let storage_dir = resolve_app_storage_dir(&window);
    let identity_path = storage_dir.join("helixgrid_identity.key");

    let identity = match VolunteerIdentity::load_or_create(&identity_path) {
        Ok(id) => {
            let pk_short = if id.public_key_hex.len() >= 24 {
                format!(
                    "{}...{}",
                    &id.public_key_hex[..16],
                    &id.public_key_hex[id.public_key_hex.len() - 8..]
                )
            } else {
                id.public_key_hex.clone()
            };
            let _ = window.emit(
                "worker-log",
                format!("  Clé Ed25519 : {} (Signatures certifiées)", pk_short),
            );
            id
        }
        Err(e) => {
            let _ = window.emit(
                "worker-log",
                format!("[!] Erreur génération identité Ed25519: {}", e),
            );
            return;
        }
    };

    let _ = window.emit(
        "worker-log",
        format!("  Maladie     : {} (PDB: {})", target_info.name, target_info.pdb),
    );
    let _ = window.emit("worker-log", format!("  Moteur IA   : {}", engine_name));
    let _ = window.emit(
        "worker-log",
        format!("  Batch Size  : {} molécules (Inférence vectorisée)", BATCH_SIZE),
    );
    let _ = window.emit(
        "worker-log",
        "  Pipeline    : Producteur-Consommateur (Génération N+1 // Inférence N)".to_string(),
    );
    let _ = window.emit(
        "worker-log",
        "  Réseau      : Expédition asynchrone Tokio + Tampon local SQLite (WAL)".to_string(),
    );

    // Résolution et garantie de présence du modèle ONNX et du récepteur 3D
    let resolved_model_path = match ensure_model_and_receptor(&window, &target_info).await {
        Ok(path) => path.to_string_lossy().to_string(),
        Err(err) => {
            let _ = window.emit(
                "worker-log",
                format!("[!] Avertissement modèle : {}. Tentative avec {}", err, model_path),
            );
            model_path
        }
    };

    let vina_found = find_vina_binary().is_some();
    let receptor_found = find_receptor_file(target_info.pdb).is_some();
    let docking_status = if vina_found && receptor_found {
        format!("AutoDock Vina v1.2.7 (Sidecar actif - PDB: {})", target_info.pdb)
    } else if vina_found {
        format!("AutoDock Vina actif (Récepteur {} manquant)", target_info.pdb)
    } else {
        "Désactivé (Binaire Vina non détecté)".to_string()
    };
    let _ = window.emit(
        "worker-log",
        format!("  Docking     : {}", docking_status),
    );

    // Initialisation du modèle ONNX avec l'accélérateur matériel adéquat
    let _ = window.emit(
        "worker-log",
        "[*] Initialisation du moteur d'inférence ONNX...".to_string(),
    );

    let (mut model, accelerator_name) = match create_session(&resolved_model_path) {
        Ok((m, acc)) => {
            let _ = window.emit(
                "worker-log",
                format!("[✓] Accélérateur matériel actif : {}", acc),
            );
            (m, acc)
        }
        Err(e) => {
            let _ = window.emit(
                "worker-log",
                format!("[!] Erreur chargement modèle ONNX: {}", e),
            );
            return;
        }
    };

    let _ = window.emit("worker-log", "=".repeat(75));

    let client = Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .unwrap_or_else(|_| Client::new());

    // Enregistrement initial de la clé publique auprès du serveur
    let reg_url = format!("{}/volunteer/register", server_url);
    let _ = client
        .post(&reg_url)
        .json(&serde_json::json!({
            "worker_id": worker_id,
            "public_key": identity.public_key_hex
        }))
        .send()
        .await;

    let mut pending_consensus_task: Option<ConsensusTaskPayload> = None;

    // 1. Synchronisation initiale avec le serveur
    let seeds_url = format!(
        "{}/seeds?worker_id={}&target={}",
        server_url, worker_id, target
    );
    if let Ok(resp) = client.get(&seeds_url).send().await {
        if resp.status().is_success() {
            if let Ok(data) = resp.json::<IslandResponse>().await {
                current_island_id = data.island_id.clone();
                current_scaffold_name = data.scaffold_name.clone();

                if let Some(srv_seeds) = data.seeds {
                    if !srv_seeds.is_empty() {
                        seeds = srv_seeds;
                    }
                }
                if let Some(mig_seeds) = data.migration_seeds {
                    migration_seeds = mig_seeds;
                }

                if let Some(task) = data.consensus_task {
                    let _ = window.emit(
                        "worker-log",
                        format!("    -> [Audit BOINC reçu] Tâche de consensus redondant : {}", task.task_id),
                    );
                    pending_consensus_task = Some(task);
                }

                let _ = window.emit(
                    "worker-log",
                    "[✓] Partitionnement coordonné (Modèle d'îlots génétiques) :".to_string(),
                );
                if let Some(ref isl) = current_island_id {
                    let scaf = current_scaffold_name.as_deref().unwrap_or("N/A");
                    let _ = window.emit(
                        "worker-log",
                        format!("    -> Îlot chimique : [{}] {}", isl, scaf),
                    );
                }
                if let Some(ref smarts) = data.scaffold_smarts {
                    let _ = window.emit(
                        "worker-log",
                        format!("    -> Noyau chimique (Scaffold) : {}", smarts),
                    );
                }
                let _ = window.emit(
                    "worker-log",
                    format!("    -> {} structures d'ancrage assignées à ce worker.", seeds.len()),
                );
                if !migration_seeds.is_empty() {
                    let _ = window.emit(
                        "worker-log",
                        format!(
                            "    -> {} graine(s) de migration reçue(s) pour diversité croisée.",
                            migration_seeds.len()
                        ),
                    );
                }
            }
        }
    } else {
        let _ = window.emit(
            "worker-log",
            "[i] Serveur central hors-ligne : utilisation de la banque pharmacophore locale autonome."
                .to_string(),
        );
    }

    // 2. Initialisation de la base SQLite locale tampon dans le stockage utilisateur
    let buffer_db_path = storage_dir.join("helixgrid_buffer.db");

    if let Err(e) = init_sqlite_buffer(&buffer_db_path) {
        let _ = window.emit(
            "worker-log",
            format!("[!] Erreur initialisation base SQLite tampon : {}", e),
        );
    } else if let Ok(pending_cnt) = count_pending_submissions(&buffer_db_path) {
        if pending_cnt > 0 {
            let _ = window.emit(
                "worker-log",
                format!(
                    "[i] Tampon SQLite local détecté : {} lot(s) en attente de synchronisation.",
                    pending_cnt
                ),
            );
        }
    }

    // 3. Démarrage de la tâche d'arrière-plan Tokio pour l'expédition non bloquante sur le réseau
    let (submission_tx, submission_rx) = tokio::sync::mpsc::channel::<BatchSubmissionPayload>(100);
    let bg_client = client.clone();
    let bg_window = window.clone();
    let bg_db_path = buffer_db_path.clone();

    tokio::spawn(async move {
        run_background_network_shipper(bg_client, bg_window, bg_db_path, submission_rx).await;
    });

    // 4. Démarrage du thread producteur (Producer-Consumer Queue)
    // Un thread dédié prépare le lot N+1 pendant que le lot N est évalué sur l'accélérateur
    let (gen_tx, mut gen_rx) = tokio::sync::mpsc::channel::<GeneratedBatch>(2);
    let shared_seeds = Arc::new(std::sync::RwLock::new(seeds.clone()));
    let shared_migration_seeds = Arc::new(std::sync::RwLock::new(migration_seeds.clone()));

    let p_seeds = Arc::clone(&shared_seeds);
    let p_migration = Arc::clone(&shared_migration_seeds);
    let p_cpu = Arc::clone(&cpu_limit);
    let p_idle_only = Arc::clone(&idle_only);
    let p_is_idle = Arc::clone(&is_idle);
    let p_stop = Arc::clone(&stop_signal);

    std::thread::Builder::new()
        .name("molecule-generator".into())
        .spawn(move || {
            producer_loop(
                gen_tx,
                p_seeds,
                p_migration,
                p_cpu,
                p_idle_only,
                p_is_idle,
                p_stop,
            );
        })
        .expect("Failed to spawn chemical generation thread");

    let mut cycle = 0usize;
    let mut total_screened_local = 0usize;
    let mut total_hits_local = 0usize;
    let hit_threshold = target_info.threshold;

    let _ = window.emit(
        "worker-log",
        format!(
            "\n[*] Lancement du pipeline Producteur-Consommateur ({}) sur {}...",
            target_info.name, accelerator_name
        ),
    );

    loop {
        // Arrêt gracieux immédiat si demandé par l'utilisateur
        if stop_signal.load(Ordering::Relaxed) {
            let _ = window.emit(
                "worker-log",
                "\n[■] Arrêt du calcul demandé par l'utilisateur.".to_string(),
            );
            break;
        }

        // Contrôle du mode économiseur d'écran (calcul uniquement lorsque inactif)
        if idle_only.load(Ordering::Relaxed) && !is_idle.load(Ordering::Relaxed) {
            let _ = window.emit("worker-idle-paused", true);
            sleep(Duration::from_millis(600)).await;
            continue;
        } else {
            let _ = window.emit("worker-idle-paused", false);
        }

        // Réception du lot préparé en continu par le thread producteur
        // Lot N+1 généré pendant que Lot N était évalué !
        let batch = match gen_rx.recv().await {
            Some(b) => b,
            None => break,
        };

        if stop_signal.load(Ordering::Relaxed) {
            break;
        }

        cycle += 1;
        let _ = window.emit(
            "worker-log",
            format!("\n--- [Cycle #{}] Lot généré (ID: #{}) ---", cycle, batch.batch_id),
        );
        let _ = window.emit(
            "worker-log",
            format!(
                "[*] {} molécules préparées en {:.2}s par le générateur N+1 (SAscore + PAINS validés)",
                batch.candidates.len(),
                batch.gen_duration_secs
            ),
        );

        // Périodiquement (tous les 10 cycles), mettre à jour les graines de migration en tâche de fond (Fire-and-forget sans bloquer le calcul)
        if cycle % 10 == 0 {
            let sync_client = client.clone();
            let sync_url = format!(
                "{}/seeds?worker_id={}&target={}",
                server_url, worker_id, target
            );
            let sync_migration = Arc::clone(&shared_migration_seeds);
            tokio::spawn(async move {
                if let Ok(resp) = sync_client.get(&sync_url).send().await {
                    if let Ok(data) = resp.json::<IslandResponse>().await {
                        if let Some(mig_seeds) = data.migration_seeds {
                            if let Ok(mut guard) = sync_migration.write() {
                                *guard = mig_seeds;
                            }
                        }
                    }
                }
            });
        }

        // Inférence IA vectorisée par batch complet sur l'accélérateur matériel
        let start_eval = Instant::now();
        let scores = match evaluate_candidates(&mut model, &batch.candidates) {
            Ok(s) => s,
            Err(e) => {
                let _ = window.emit("worker-log", format!("[!] Erreur inférence: {}", e));
                sleep(Duration::from_secs(1)).await;
                continue;
            }
        };
        let eval_time = start_eval.elapsed().as_secs_f64();
        total_screened_local += batch.candidates.len();

        let max_score = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let mean_score = if !scores.is_empty() {
            scores.iter().sum::<f32>() / scores.len() as f32
        } else {
            0.0
        };

        let _ = window.emit(
            "worker-log",
            format!(
                "    -> Inférence vectorisée sur {} ({} molécules) en {:.3}s (pKd moyen: {:.2} | max: {:.2})",
                accelerator_name,
                batch.candidates.len(),
                eval_time,
                mean_score,
                max_score
            ),
        );

        // Détection et qualification des pépites candidates
        let mut confirmed_hits = Vec::new();

        for (cand, &score) in batch.candidates.iter().zip(scores.iter()) {
            if score >= hit_threshold || (score >= 8.2 && score == max_score) {
                // Validation physique structure-based par AutoDock Vina (Sidecar)
                let dock_res = run_docking(&cand.mol, &target_info, 1);
                let docking_kcal = dock_res.as_ref().map(|d| d.affinity_kcal);
                let sdf_3d = molecule_to_sdf_3d(&cand.mol, &cand.smiles);

                let pains_str = if cand.pains_ok {
                    "✓ PAINS free"
                } else {
                    "⚠ PAINS alert"
                };
                let dg_str = docking_kcal
                    .map(|dg| format!(" | ΔG Vina: {:.2} kcal/mol", dg))
                    .unwrap_or_default();

                let _ = window.emit(
                    "worker-log",
                    format!(
                        "    ★ PÉPITE QUALIFIÉE : {}... | pKd: {:.2}{} | SAscore: {:.2} | {}",
                        &cand.smiles.chars().take(34).collect::<String>(),
                        score,
                        dg_str,
                        cand.sascore,
                        pains_str
                    ),
                );

                confirmed_hits.push(MoleculeCandidatePayload {
                    smiles: cand.smiles.clone(),
                    predicted_pkd: score,
                    mw: Some(cand.mw),
                    logp: Some(cand.logp),
                    docking_kcal,
                    sascore: Some(cand.sascore),
                    pains_ok: Some(cand.pains_ok),
                    brenk_ok: Some(cand.brenk_ok),
                    scaffold: current_scaffold_name.clone(),
                    island_id: current_island_id.clone(),
                    sdf_3d,
                });

                // Enrichir immédiatement le pool de graines partagé pour que le thread producteur
                // en dérive de nouvelles variantes dès les prochains lots
                if !seeds.contains(&cand.smiles) {
                    seeds.push(cand.smiles.clone());
                    if let Ok(mut guard) = shared_seeds.write() {
                        if !guard.contains(&cand.smiles) {
                            guard.push(cand.smiles.clone());
                        }
                    }
                }
            }
        }

        if !confirmed_hits.is_empty() {
            total_hits_local += confirmed_hits.len();
        }

        // Émission de la molécule vedette pour visualisation 2D (SmilesDrawer) & 3D (3Dmol.js)
        let best_candidate_idx = scores
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(idx, _)| idx);

        if let Some(idx) = best_candidate_idx {
            let top_cand = &batch.candidates[idx];
            let top_score = scores[idx];
            let is_hit = top_score >= hit_threshold || (top_score >= 8.2 && top_score == max_score);

            let _ = window.emit(
                "molecule-discovered",
                MoleculeDiscoveryPayload {
                    smiles: top_cand.smiles.clone(),
                    score: top_score,
                    mw: top_cand.mw,
                    logp: top_cand.logp,
                    sascore: top_cand.sascore,
                    pains_ok: top_cand.pains_ok,
                    brenk_ok: top_cand.brenk_ok,
                    is_hit,
                    target: target.clone(),
                    engine: engine.clone(),
                    scaffold: current_scaffold_name.clone(),
                    island_id: current_island_id.clone(),
                },
            );
        }

        // Émission des métriques pour la gamification, le profil et l'allocation
        let current_cpu_pct = cpu_limit.load(Ordering::Relaxed).clamp(10, 100);
        let _ = window.emit(
            "worker-metrics",
            WorkerMetricsPayload {
                cycle,
                screened_batch: batch.candidates.len(),
                screened_total: total_screened_local,
                total_hits_local,
                target: target.clone(),
                cpu_limit: current_cpu_pct,
                idle_only: idle_only.load(Ordering::Relaxed),
                is_idle: is_idle.load(Ordering::Relaxed),
            },
        );

        // Traitement de la tâche d'audit consensus BOINC / Folding@home si attribuée
        let mut consensus_resp_to_send: Option<ConsensusResponsePayload> = None;
        if let Some(canary) = pending_consensus_task.take() {
            if let Ok(c_mol) = parse(&canary.smiles) {
                let cand = CandidateMol {
                    smiles: canary.smiles.clone(),
                    mol: c_mol.clone(),
                    mw: chematic::chem::molecular_weight(&c_mol) as f32,
                    logp: chematic::chem::logp_crippen(&c_mol) as f32,
                    sascore: chematic::chem::sa_score(&c_mol) as f32,
                    pains_ok: true,
                    brenk_ok: true,
                };
                if let Ok(c_scores) = evaluate_candidates(&mut model, &[cand]) {
                    let c_pkd = c_scores.first().copied().unwrap_or(6.0);
                    let mut c_dock: Option<f32> = None;
                    if c_pkd >= 7.8 {
                        if let Some(res) = run_docking(&c_mol, &target_info, 1) {
                            c_dock = Some(res.affinity_kcal);
                        }
                    }
                    let _ = window.emit(
                        "worker-log",
                        format!("[*] Audit Consensus BOINC calculé pour {} : pKd={:.2}", canary.task_id, c_pkd),
                    );
                    consensus_resp_to_send = Some(ConsensusResponsePayload {
                        task_id: canary.task_id,
                        predicted_pkd: c_pkd,
                        docking_kcal: c_dock,
                    });
                }
            }
        }

        // Expédition NON BLOQUANTE des résultats certifiés avec signature Ed25519
        let now_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        let canonical_msg = format!(
            "{}:{}:{}:{}",
            worker_id,
            now_ts as u64,
            batch.candidates.len(),
            confirmed_hits.len()
        );
        let signature_hex = identity.sign_message(canonical_msg.as_bytes());

        let payload = BatchSubmissionPayload {
            worker_id: worker_id.clone(),
            evaluated_count: batch.candidates.len(),
            hits: confirmed_hits,
            target: Some(target.clone()),
            engine: Some(engine.clone()),
            island_id: current_island_id.clone(),
            public_key: Some(identity.public_key_hex.clone()),
            signature: Some(signature_hex),
            timestamp: Some(now_ts),
            consensus_response: consensus_resp_to_send,
        };

        // Expédition asynchrone Fire-and-forget via canal Tokio mpsc :
        // Le calcul continue immédiatement sans jamais attendre le serveur HTTP
        match submission_tx.try_send(payload) {
            Ok(()) => {}
            Err(tokio::sync::mpsc::error::TrySendError::Full(p)) => {
                // File mpsc temporairement saturée : mise en tampon SQLite locale immédiate sans bloquer
                let _ = save_pending_submission(&buffer_db_path, &p);
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                let _ = window.emit(
                    "worker-log",
                    "[!] Canal d'expédition réseau d'arrière-plan fermé".to_string(),
                );
            }
        }

        // Modulation duty-cycle CPU proportionnelle uniquement si l'utilisateur demande une réduction (< 100%)
        // Si CPU à 100% : 0 ms de pause (yield coopératif sans sleep) pour un débit continu maximal
        if current_cpu_pct < 100 {
            let throttle_ms = ((eval_time * 1000.0) as u64)
                * (100 - current_cpu_pct as u64)
                / (current_cpu_pct as u64);
            if throttle_ms > 0 {
                sleep(Duration::from_millis(throttle_ms)).await;
            }
        } else {
            tokio::task::yield_now().await;
        }
    }

    stop_signal.store(true, Ordering::SeqCst);
    is_running.store(false, Ordering::SeqCst);
    let _ = window.emit("worker-stopped", ());
    let _ = window.emit(
        "worker-log",
        "[✓] Session de calcul arrêtée proprement.".to_string(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chem_generation_and_filters() {
        let mol = parse("c1ccccc1").unwrap();
        let smiles_out = canonical_smiles(&mol);
        assert_eq!(smiles_out, "c1ccccc1");

        let mw = chematic::chem::molecular_weight(&mol);
        let logp = chematic::chem::logp_crippen(&mol);
        let sascore = chematic::chem::sa_score(&mol);
        let pains = chematic::chem::alerts::pains_passes(&mol);
        let brenk = chematic::chem::alerts::brenk_passes(&mol);
        println!(
            "Benzène -> MW: {}, LogP: {}, SA: {}, PAINS: {}, Brenk: {}",
            mw, logp, sascore, pains, brenk
        );

        let seeds = vec!["c1ccccc1".to_string(), "COc1ccccc1".to_string()];
        let mutants = generate_molecules(&seeds, 10);
        println!("Generated {} mutants", mutants.len());
        assert!(!mutants.is_empty());
        for m in &mutants {
            println!("  Mutant: {} | MW: {} | LogP: {}", m.smiles, m.mw, m.logp);
        }
    }

    #[test]
    fn test_onnx_hardware_acceleration() {
        let model_path = "affinity_model.onnx";
        if !std::path::Path::new(model_path).exists() {
            println!("Model file not found at {}, skipping test", model_path);
            return;
        }

        let (mut session, acc_name) =
            create_session(model_path).expect("Failed to create ONNX session");
        println!("Session loaded successfully with accelerator: {}", acc_name);

        let mol = parse("COc1cc2ncnc(Nc3ccc(F)c(Cl)c3)c2cc1OCC").unwrap();
        let cand = CandidateMol {
            smiles: "COc1cc2ncnc(Nc3ccc(F)c(Cl)c3)c2cc1OCC".to_string(),
            mol,
            mw: 446.9,
            logp: 4.1,
            sascore: 2.8,
            pains_ok: true,
            brenk_ok: true,
        };

        let scores = evaluate_candidates(&mut session, &[cand]).expect("Evaluation failed");
        assert_eq!(scores.len(), 1);
        println!("Predicted pKd score: {}", scores[0]);
    }

    #[test]
    fn test_sqlite_local_buffer_persistence() {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join(format!("test_buffer_{}.db", uuid::Uuid::new_v4().as_simple()));
        init_sqlite_buffer(&db_path).expect("Failed to init SQLite buffer");

        assert_eq!(count_pending_submissions(&db_path).unwrap(), 0);

        let payload = BatchSubmissionPayload {
            worker_id: "test_worker_persistent".to_string(),
            evaluated_count: 50,
            hits: vec![
                MoleculeCandidatePayload {
                    smiles: "c1ccccc1".to_string(),
                    predicted_pkd: 8.95,
                    mw: Some(78.1),
                    logp: Some(2.1),
                    docking_kcal: None,
                    sascore: Some(1.2),
                    pains_ok: Some(true),
                    brenk_ok: Some(true),
                    scaffold: Some("benzene".to_string()),
                    island_id: Some("island_1".to_string()),
                    sdf_3d: None,
                }
            ],
            target: Some("EGFR_T790M".to_string()),
            engine: Some("generative_evolution".to_string()),
            island_id: Some("island_1".to_string()),
            public_key: None,
            signature: None,
            timestamp: None,
            consensus_response: None,
        };

        let row_id = save_pending_submission(&db_path, &payload).expect("Failed to save submission");
        assert!(row_id > 0);
        assert_eq!(count_pending_submissions(&db_path).unwrap(), 1);

        let pending = get_pending_submissions(&db_path, 10).expect("Failed to retrieve pending");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].1.worker_id, "test_worker_persistent");
        assert_eq!(pending[0].1.hits.len(), 1);
        assert_eq!(pending[0].1.hits[0].predicted_pkd, 8.95);

        delete_pending_submission(&db_path, pending[0].0).expect("Failed to delete submission");
        assert_eq!(count_pending_submissions(&db_path).unwrap(), 0);

        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(format!("{}-wal", db_path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", db_path.display()));
    }

    #[tokio::test]
    async fn test_producer_consumer_queue() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<GeneratedBatch>(2);
        let seeds = Arc::new(std::sync::RwLock::new(vec![
            "COc1cc2ncnc(Nc3ccc(F)c(Cl)c3)c2cc1OCC".to_string(),
            "c1ccc2ncnc(Nc3ccc(Br)cc3)c2c1".to_string(),
            "CN1CCN(CC1)C(=O)C2=CC=C(C=C2)NC(=O)C3=CC=CC=C3".to_string(),
        ]));
        let migration_seeds = Arc::new(std::sync::RwLock::new(Vec::new()));
        let cpu_limit = Arc::new(AtomicU32::new(100));
        let idle_only = Arc::new(AtomicBool::new(false));
        let is_idle = Arc::new(AtomicBool::new(true));
        let stop_signal = Arc::new(AtomicBool::new(false));

        let p_seeds = Arc::clone(&seeds);
        let p_migration = Arc::clone(&migration_seeds);
        let p_cpu = Arc::clone(&cpu_limit);
        let p_idle_only = Arc::clone(&idle_only);
        let p_is_idle = Arc::clone(&is_idle);
        let p_stop = Arc::clone(&stop_signal);

        let handle = std::thread::spawn(move || {
            producer_loop(tx, p_seeds, p_migration, p_cpu, p_idle_only, p_is_idle, p_stop);
        });

        // Vérifier la réception d'au moins 2 lots préparés
        let batch1 = rx.recv().await.expect("Batch 1 should be received");
        assert_eq!(batch1.batch_id, 1);
        assert!(!batch1.candidates.is_empty());

        let batch2 = rx.recv().await.expect("Batch 2 should be received");
        assert_eq!(batch2.batch_id, 2);
        assert!(!batch2.candidates.is_empty());

        stop_signal.store(true, Ordering::Relaxed);
        drop(rx);
        let _ = handle.join();
    }

    #[test]
    fn test_offline_buffering_resilience() {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join(format!("test_offline_buffer_{}.db", uuid::Uuid::new_v4().as_simple()));
        init_sqlite_buffer(&db_path).expect("Failed to init SQLite buffer");

        for i in 1..=3 {
            let payload = BatchSubmissionPayload {
                worker_id: format!("node_{}", i),
                evaluated_count: 50,
                hits: vec![],
                target: Some("EGFR_T790M".to_string()),
                engine: Some("generative_evolution".to_string()),
                island_id: Some("island_0".to_string()),
                public_key: None,
                signature: None,
                timestamp: None,
                consensus_response: None,
            };
            save_pending_submission(&db_path, &payload).expect("Failed to save submission");
        }

        assert_eq!(count_pending_submissions(&db_path).unwrap(), 3);

        let pending = get_pending_submissions(&db_path, 2).expect("Query failed");
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].1.worker_id, "node_1");
        assert_eq!(pending[1].1.worker_id, "node_2");

        // Supprimer le premier lot comme s'il avait été transmis
        delete_pending_submission(&db_path, pending[0].0).expect("Delete failed");
        assert_eq!(count_pending_submissions(&db_path).unwrap(), 2);

        let remaining = get_pending_submissions(&db_path, 10).expect("Query failed");
        assert_eq!(remaining.len(), 2);
        assert_eq!(remaining[0].1.worker_id, "node_2");
        assert_eq!(remaining[1].1.worker_id, "node_3");

        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(format!("{}-wal", db_path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", db_path.display()));
    }

    #[test]
    fn test_native_vina_docking_and_3d_generation() {
        let smiles = "COc1cc2ncnc(Nc3ccc(F)c(Cl)c3)c2cc1OCC";
        let mol = parse(smiles).expect("Failed to parse SMILES");

        // 1. Conformation 3D et export SDF
        let sdf = molecule_to_sdf_3d(&mol, "Gefitinib_Derivative");
        assert!(sdf.is_some(), "3D conformer generation should succeed");
        let sdf_str = sdf.unwrap();
        assert!(sdf_str.contains("M  END"), "SDF output must be valid V2000");

        // 2. Détection du binaire Vina
        let vina_path = find_vina_binary();
        println!("Vina binary path: {:?}", vina_path);
        assert!(vina_path.is_some(), "Vina binary should be found in dev or bundle paths");

        // 3. Détection du fichier récepteur
        let receptor_path = find_receptor_file("2JIT");
        println!("Receptor path (2JIT): {:?}", receptor_path);
        assert!(receptor_path.is_some(), "Receptor 2JIT.pdbqt should be located");

        // 4. Exécution du docking physique
        let target_config = get_target_config("EGFR_T790M");
        let docking_result = run_docking(&mol, &target_config, 1);
        assert!(docking_result.is_some(), "Docking calculation should succeed");

        let res = docking_result.unwrap();
        println!(
            "Docking free energy ΔG: {:.2} kcal/mol (Favorable: {})",
            res.affinity_kcal, res.is_favorable
        );
        assert!(res.affinity_kcal < 0.0, "Physical binding affinity should be negative");
    }

    #[test]
    fn test_brics_retrosynthetic_fragment_crossover() {
        let parent1_smi = "COc1cc2ncnc(Nc3ccc(F)c(Cl)c3)c2cc1OCC"; // Gefitinib-like
        let parent2_smi = "c1ccc(cc1)N2CCOCC2"; // Phenylmorpholine
        let m1 = parse(parent1_smi).expect("Failed to parse Parent 1");
        let m2 = parse(parent2_smi).expect("Failed to parse Parent 2");

        // 1. Détection des liaisons éligibles au crossover BRICS
        let bonds1 = find_crossover_bonds(&m1);
        let bonds2 = find_crossover_bonds(&m2);
        println!("Crossover breakable bonds Parent 1: {}", bonds1.len());
        println!("Crossover breakable bonds Parent 2: {}", bonds2.len());
        assert!(!bonds1.is_empty(), "Parent 1 should have breakable bonds");
        assert!(!bonds2.is_empty(), "Parent 2 should have breakable bonds");

        // 2. Croisement direct de deux parents
        let children = crossover_parents(&m1, &m2);
        println!("Generated {} crossover children", children.len());
        assert!(!children.is_empty(), "Crossover should produce children");

        for (i, child) in children.iter().enumerate() {
            let smi = canonical_smiles(child);
            let mw = chematic::chem::molecular_weight(child);
            let logp = chematic::chem::logp_crippen(child);
            let sa = chematic::chem::sa_score(child);
            println!(
                "  Child {}: {} | MW: {:.1} | LogP: {:.2} | SA: {:.2}",
                i + 1,
                smi,
                mw,
                logp,
                sa
            );
            assert_ne!(smi, parent1_smi, "Child must differ from Parent 1");
            assert_ne!(smi, parent2_smi, "Child must differ from Parent 2");
        }

        // 3. Test de génération globale avec réservoir de scaffolds
        let seeds = vec![parent1_smi.to_string()];
        let mutants = generate_molecules(&seeds, 20);
        println!(
            "Generated {} molecules via BRICS crossover & mutations",
            mutants.len()
        );
        assert_eq!(mutants.len(), 20, "Should generate exactly requested count");

        // Vérifier l'apparition de scaffolds variés et inédits
        let mut unique_scaffolds = HashSet::new();
        for m in &mutants {
            assert!(
                m.mw >= 160.0 && m.mw <= 700.0,
                "MW out of bounds: {}",
                m.mw
            );
            assert!(
                m.logp >= -2.0 && m.logp <= 6.0,
                "LogP out of bounds: {}",
                m.logp
            );
            assert!(m.sascore <= 5.5, "SAscore too high: {}", m.sascore);
            assert!(m.pains_ok, "Must pass PAINS");
            unique_scaffolds.insert(m.smiles.clone());
        }
        println!("Unique generated structures: {}", unique_scaffolds.len());
        assert!(
            unique_scaffolds.len() >= 15,
            "Generative diversity must be high"
        );
    }

    #[test]
    fn test_ed25519_identity_and_signature_verification() {
        let temp_dir = std::env::temp_dir();
        let key_path = temp_dir.join(format!("test_identity_{}.key", uuid::Uuid::new_v4().as_simple()));

        // 1. Création de la clé cryptographique
        let id = VolunteerIdentity::load_or_create(&key_path).expect("Failed to create identity");
        assert_eq!(id.public_key_hex.len(), 64);
        assert!(key_path.exists());

        // 2. Signature d'un message canonique de lot
        let canonical_msg = "test_volunteer:1773342000:50:3";
        let sig_hex = id.sign_message(canonical_msg.as_bytes());
        assert_eq!(sig_hex.len(), 128);

        // 3. Vérification de la signature authentique
        let is_valid = VolunteerIdentity::verify_signature(
            &id.public_key_hex,
            canonical_msg.as_bytes(),
            &sig_hex,
        );
        assert!(is_valid, "Authentic signature must be valid");

        // 4. Détection immédiate d'une falsification de message (anti-fraude)
        let forged_msg = "test_volunteer:1773342000:50:4";
        let is_forged_valid = VolunteerIdentity::verify_signature(
            &id.public_key_hex,
            forged_msg.as_bytes(),
            &sig_hex,
        );
        assert!(!is_forged_valid, "Forged payload must be rejected");

        // 5. Rechargement de la clé persistée
        let id_reloaded = VolunteerIdentity::load_or_create(&key_path).expect("Failed to reload identity");
        assert_eq!(id.public_key_hex, id_reloaded.public_key_hex);

        let _ = std::fs::remove_file(&key_path);
    }

    #[test]
    fn test_models_and_receptors_discovery() {
        // 1. Vérification de la présence et détection de tous les modèles ONNX
        let m1 = find_model_file("affinity_model.onnx");
        assert!(m1.is_some(), "affinity_model.onnx should be found");

        let m2 = find_model_file("model_parkinson_lrrk2.onnx");
        assert!(m2.is_some(), "model_parkinson_lrrk2.onnx should be found");

        let m3 = find_model_file("model_antibiotic_pknb.onnx");
        assert!(m3.is_some(), "model_antibiotic_pknb.onnx should be found");

        // 2. Vérification de la détection de tous les récepteurs 3D PDBQT
        for pdb in &["1FKN", "2JIT", "3PP0", "7BQ7", "8T5V"] {
            let r = find_receptor_file(pdb);
            assert!(r.is_some(), "Receptor {}.pdbqt should be located", pdb);
        }

        // 3. Vérification des liaisons modèle-cible
        let parkinson_cfg = get_target_config("Parkinson_LRRK2");
        assert_eq!(parkinson_cfg.model_file, "model_parkinson_lrrk2.onnx");
        assert_eq!(parkinson_cfg.pdb, "3PP0");

        let tb_cfg = get_target_config("PKNB_Tuberculosis");
        assert_eq!(tb_cfg.model_file, "model_antibiotic_pknb.onnx");
        assert_eq!(tb_cfg.pdb, "8T5V");
    }

    #[test]
    fn test_specialized_models_inference() {
        for model_name in &["affinity_model.onnx", "model_parkinson_lrrk2.onnx", "model_antibiotic_pknb.onnx"] {
            let model_path = find_model_file(model_name).expect("Model should exist");
            let (mut session, acc_name) = create_session(model_path.to_str().unwrap()).expect("Session creation should succeed");
            println!("Loaded {} with {}", model_name, acc_name);

            let mol = parse("COc1cc2ncnc(Nc3ccc(F)c(Cl)c3)c2cc1OCC").unwrap();
            let cand = CandidateMol {
                smiles: "COc1cc2ncnc(Nc3ccc(F)c(Cl)c3)c2cc1OCC".to_string(),
                mol,
                mw: 446.9,
                logp: 4.1,
                sascore: 2.8,
                pains_ok: true,
                brenk_ok: true,
            };

            let scores = evaluate_candidates(&mut session, &[cand]).expect("Evaluation should succeed");
            assert_eq!(scores.len(), 1);
            println!("  Predicted score for {}: {:.2}", model_name, scores[0]);
        }
    }
}
