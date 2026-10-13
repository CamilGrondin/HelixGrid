const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// URL du serveur central HelixGrid (Production HTTPS par défaut, avec fallback configurable)
const DEFAULT_REMOTE_SERVER = "https://srv1091727.hstgr.cloud";
let SERVER_URL = localStorage.getItem('helixgrid_server_url') || DEFAULT_REMOTE_SERVER;

// ---------------------------------------------------------------------------
// Base de connaissances des cibles & poches catalytiques (3D PDB)
// ---------------------------------------------------------------------------
const TARGET_DETAILS = {
  "EGFR_T790M": {
    name: "🫁 Cancer du Poumon (EGFR T790M Résistant)",
    pdb: "2JIT",
    desc: "Cible : Domaine kinase du récepteur EGFR portant la mutation de résistance T790M. L'objectif est de générer des inhibiteurs de 3e et 4e génération capables de contourner la résistance acquise aux thérapies ciblées dans le cancer du poumon.",
    pocketCenter: { x: -2.03, y: 32.91, z: 8.62 },
    pocketResidues: [718, 745, 790, 793, 797, 855],
    diseaseCategory: "cancer"
  },
  "SARS_CoV_2_Mpro": {
    name: "🦠 Covid-19 / Antiviral (Protéase Mpro)",
    pdb: "7BQ7",
    desc: "Cible : Protéase principale (3CLpro / Mpro) du SARS-CoV-2. Essentielle au clivage des polyprotéines virales. L'objectif est de concevoir des inhibiteurs compétitifs ou covalents bloquant la réplication virale.",
    pocketCenter: { x: -76.94, y: 25.68, z: 21.37 },
    pocketResidues: [41, 145, 163, 166, 189],
    diseaseCategory: "antiviral"
  },
  "KRAS_G12D": {
    name: "🎗️ Cancer Pancréas & Côlon (Mutant KRAS G12D)",
    pdb: "8T5V",
    desc: "Cible : Oncoprotéine KRAS avec mutation oncogénique G12D, longtemps considérée comme 'indruggable'. Présente dans 90% des adénocarcinomes pancréatiques et 45% des cancers colorectaux.",
    pocketCenter: { x: 12.62, y: 6.73, z: -10.87 },
    pocketResidues: [12, 13, 61, 68, 95, 96],
    diseaseCategory: "cancer"
  },
  "BACE1_Alzheimer": {
    name: "🧠 Maladie d'Alzheimer (BACE-1 Protéase)",
    pdb: "1FKN",
    desc: "Cible : Bêta-sécrétase 1 (BACE-1), enzyme clé dans le clivage de l'APP et l'accumulation des plaques amyloïdes neurotoxiques responsables de la dégénérescence synaptique.",
    pocketCenter: { x: 27.26, y: 18.07, z: 21.56 },
    pocketResidues: [32, 73, 108, 228, 289],
    diseaseCategory: "neuro"
  },
  "HER2_Breast": {
    name: "🔬 Cancer du Sein (Récepteur HER2 / ERBB2)",
    pdb: "3PP0",
    desc: "Cible : Récepteur tyrosine-kinase HER2 surexprimé dans 20% des cancers du sein agressifs. L'objectif est de concevoir de petites molécules sélectives pénétrant la barrière hémato-encéphalique.",
    pocketCenter: { x: 26.48, y: 30.93, z: 7.01 },
    pocketResidues: [753, 798, 801, 863],
    diseaseCategory: "cancer"
  },
  "Parkinson_LRRK2": {
    name: "🧠 Maladie de Parkinson (Kinase LRRK2 G2019S)",
    pdb: "3PP0",
    desc: "Cible : Kinase LRRK2 (Leucine-Rich Repeat Kinase 2) avec mutation hyperactive G2019S, cause génétique majeure de la maladie de Parkinson. L'inhibition sélective vise à freiner la neurodégénérescence dopaminergique.",
    pocketCenter: { x: 26.48, y: 30.93, z: 7.01 },
    pocketResidues: [753, 798, 801, 863],
    diseaseCategory: "neuro"
  },
  "PKNB_Tuberculosis": {
    name: "💊 Antibiorésistance / Tuberculose (Kinase PknB)",
    pdb: "8T5V",
    desc: "Cible : Sérine/thréonine kinase PknB de Mycobacterium tuberculosis, régulateur vital de la division et paroi bactérienne. Cible clé pour contrer les souches multirésistantes (MDR/XDR).",
    pocketCenter: { x: 16.18, y: 13.87, z: -2.42 },
    pocketResidues: [40, 95, 142, 166],
    diseaseCategory: "antiviral"
  }
};

// ---------------------------------------------------------------------------
// Références DOM
// ---------------------------------------------------------------------------
const startBtn = document.getElementById('start-btn');
const logContainer = document.getElementById('log-container');
const diseaseSelect = document.getElementById('disease-select');
const engineSelect = document.getElementById('engine-select');
const targetTitleEl = document.getElementById('target-title');
const pdbBadgeEl = document.getElementById('pdb-badge');
const targetDescBox = document.getElementById('target-desc-box');
const viewerContainer = document.getElementById('viewer');
const viewerLoading = document.getElementById('viewer-loading');

// Outils 3D
const viewPocketBtn = document.getElementById('view-pocket-btn');
const viewGlobalBtn = document.getElementById('view-global-btn');
const toggleSurfaceBtn = document.getElementById('toggle-surface-btn');
const toggleSpinBtn = document.getElementById('toggle-spin-btn');

// Carte 2D
const mol2dCanvas = document.getElementById('mol-2d-canvas');
const molPlaceholder = document.getElementById('mol-placeholder');
const molMetaPkd = document.getElementById('mol-meta-pkd');
const molMetaMw = document.getElementById('mol-meta-mw');
const molMetaLogp = document.getElementById('mol-meta-logp');
const molMetaSascore = document.getElementById('mol-meta-sascore');
const molSmilesText = document.getElementById('mol-smiles-text');
const copySmiBtn = document.getElementById('copy-smi-btn');
const molStatusBadge = document.getElementById('mol-status-badge');

// Contrôles CPU
const cpuSlider = document.getElementById('cpu-slider');
const cpuSliderVal = document.getElementById('cpu-slider-val');
const presetButtons = document.querySelectorAll('.preset-btn');
const idleOnlyToggle = document.getElementById('idle-only-toggle');
const resourceStatusPill = document.getElementById('resource-status-pill');

// Leaderboard
const lbFilterTarget = document.getElementById('lb-filter-target');
const lbRefreshBtn = document.getElementById('lb-refresh-btn');
const lbExportSdfBtn = document.getElementById('lb-export-sdf-btn');
const lbHitsList = document.getElementById('leaderboard-hits-list');
const lbStatScreened = document.getElementById('lb-stat-screened');
const lbStatHits = document.getElementById('lb-stat-hits');
const lbStatBest = document.getElementById('lb-stat-best');
const lbStatWorkers = document.getElementById('lb-stat-workers');

// Profil & Gamification
const volunteerPseudoInput = document.getElementById('volunteer-pseudo-input');
const savePseudoBtn = document.getElementById('save-pseudo-btn');
const volunteerTierBadge = document.getElementById('volunteer-tier-badge');
const userStatScreened = document.getElementById('user-stat-screened');
const userStatHits = document.getElementById('user-stat-hits');
const userStatTime = document.getElementById('user-stat-time');
const userStatEnergy = document.getElementById('user-stat-energy');
const impactBarCancer = document.getElementById('impact-bar-cancer');
const impactPctCancer = document.getElementById('impact-pct-cancer');
const impactBarNeuro = document.getElementById('impact-bar-neuro');
const impactPctNeuro = document.getElementById('impact-pct-neuro');
const impactBarAntiviral = document.getElementById('impact-bar-antiviral');
const impactPctAntiviral = document.getElementById('impact-pct-antiviral');
const badgesUnlockedCount = document.getElementById('badges-unlocked-count');
const toastContainer = document.getElementById('toast-container');

// Bannière mise à jour
const updateBanner = document.getElementById('update-banner');
const updateBtn = document.getElementById('update-btn');

// Paramètres & Système
const settingPreventSleep = document.getElementById('setting-prevent-sleep');
const settingKeepTray = document.getElementById('setting-keep-tray');
const settingsPseudoInput = document.getElementById('settings-pseudo-input');
const settingsSavePseudoBtn = document.getElementById('settings-save-pseudo-btn');
const settingsPubkeyDisplay = document.getElementById('settings-pubkey-display');
const settingsCopyPubkeyBtn = document.getElementById('settings-copy-pubkey-btn');
const settingsBufferStatus = document.getElementById('settings-buffer-status');
const settingsOpenDirBtn = document.getElementById('settings-open-dir-btn');
const settingsServerUrlInput = document.getElementById('settings-server-url-input');
const settingsTestServerBtn = document.getElementById('settings-test-server-btn');
const settingsSaveServerBtn = document.getElementById('settings-save-server-btn');
const settingsResetServerBtn = document.getElementById('settings-reset-server-btn');
const settingsServerStatus = document.getElementById('settings-server-status');

// ---------------------------------------------------------------------------
// État Global de l'application
// ---------------------------------------------------------------------------
let glViewer = null;
let currentLigandModel = null;
let pocketSurface = null;
let isSpinning = false;
let showPocketSurface = true;
let smilesDrawer = null;
let workerStarted = false;

let currentCpuAllocation = 100;
let isIdleOnly = false;
let isUserIdle = false;
let lastActivityTime = Date.now();

let volunteerPseudo = localStorage.getItem('helixgrid_volunteer_pseudo') || '';
let userTotalScreened = parseInt(localStorage.getItem('helixgrid_user_screened') || '0', 10);
let userTotalHits = parseInt(localStorage.getItem('helixgrid_user_hits') || '0', 10);
let userComputeSeconds = parseInt(localStorage.getItem('helixgrid_user_seconds') || '0', 10);
let diseaseEfforts = JSON.parse(localStorage.getItem('helixgrid_disease_efforts') || '{"cancer": 0, "neuro": 0, "antiviral": 0}');
let unlockedBadges = new Set(JSON.parse(localStorage.getItem('helixgrid_unlocked_badges') || '[]'));

let currentDisplayedSmiles = '';

// ---------------------------------------------------------------------------
// Notifications Toast
// ---------------------------------------------------------------------------
function showToast(message, type = 'info') {
  const toast = document.createElement('div');
  toast.className = `toast ${type}`;
  toast.innerText = message;
  toastContainer.appendChild(toast);
  setTimeout(() => {
    if (toast.parentElement) toast.remove();
  }, 4000);
}

// ---------------------------------------------------------------------------
// Gestion des Onglets
// ---------------------------------------------------------------------------
document.querySelectorAll('.tab-btn').forEach(btn => {
  btn.addEventListener('click', () => {
    document.querySelectorAll('.tab-btn').forEach(b => b.classList.remove('active'));
    document.querySelectorAll('.tab-content').forEach(c => c.classList.remove('active'));

    btn.classList.add('active');
    const tabId = btn.getAttribute('data-tab');
    const targetContent = document.getElementById(tabId);
    if (targetContent) targetContent.classList.add('active');

    if (tabId === 'tab-leaderboard') {
      fetchLeaderboard();
    }
    if (tabId === 'tab-settings') {
      loadAccountSettings();
    }
  });
});

// ---------------------------------------------------------------------------
// 1. VISUALISATION 3D DYNAMIQUE AVEC 3Dmol.js (PROTÉINE + POCHE + LIGAND)
// ---------------------------------------------------------------------------
function loadPdbInViewer(pdbId) {
  if (viewerLoading) viewerLoading.style.display = 'block';

  try {
    viewerContainer.innerHTML = '';
    currentLigandModel = null;
    pocketSurface = null;

    glViewer = $3Dmol.createViewer(viewerContainer, {
      backgroundColor: '#070709'
    });

    const targetKey = diseaseSelect.value;
    const info = TARGET_DETAILS[targetKey] || TARGET_DETAILS["EGFR_T790M"];

    $3Dmol.download(`pdb:${pdbId}`, glViewer, {}, function () {
      // 1. Structure cartoon globale de la protéine
      glViewer.setStyle({}, {
        cartoon: { color: 'spectrum', opacity: 0.82 }
      });

      // 2. Co-facteurs ou ligands cristallins existants
      glViewer.setStyle({ hetflag: true }, {
        stick: { colorscheme: 'greenCarbon', radius: 0.22 },
        sphere: { scale: 0.2 }
      });

      // 3. Highlight spécifique de la poche catalytique
      highlightPocket(info);

      glViewer.zoomTo();
      glViewer.render();
      if (viewerLoading) viewerLoading.style.display = 'none';
    });
  } catch (err) {
    console.error("Erreur 3Dmol:", err);
    if (viewerLoading) viewerLoading.style.display = 'none';
  }
}

function highlightPocket(info) {
  if (!glViewer || !info || !info.pocketResidues) return;
  const pocketSel = { resi: info.pocketResidues };

  // 1. Résidus clés formant la poche active en bâtonnets stylisés avec couleurs amino
  glViewer.setStyle(pocketSel, {
    cartoon: { color: '#38bdf8', opacity: 0.95 },
    stick: { colorscheme: 'amino', radius: 0.22 }
  });

  // 2. Coquille de résidus en contact immédiat (4.5 Å)
  glViewer.setStyle({
    within: { distance: 4.5, sel: pocketSel }
  }, {
    stick: { colorscheme: 'amino', radius: 0.14 }
  });

  // 3. Surface de la cavité catalytique
  if (showPocketSurface) {
    try {
      if (pocketSurface) glViewer.removeSurface(pocketSurface);
      pocketSurface = glViewer.addSurface($3Dmol.SurfaceType.VDW, {
        opacity: 0.32,
        color: '#2563eb'
      }, {
        within: { distance: 3.8, sel: pocketSel }
      });
    } catch (_) {}
  }
}

// Positionne et affiche la molécule candidate dans la poche active 3D
async function displayLigandInPocket(moleculeData) {
  if (!glViewer || !moleculeData || !moleculeData.smiles) return;

  const targetKey = moleculeData.target || diseaseSelect.value;
  const info = TARGET_DETAILS[targetKey] || TARGET_DETAILS["EGFR_T790M"];
  const pocketCenter = info.pocketCenter;

  let sdfBlock = moleculeData.sdf_3d;

  // Si le bloc SDF n'est pas fourni localement, on tente de le récupérer depuis le serveur
  if (!sdfBlock) {
    try {
      sdfBlock = await invoke('fetch_molecule_sdf', { smilesOrId: encodeURIComponent(moleculeData.smiles) });
    } catch (_) {
      try {
        const resp = await fetch(`${SERVER_URL}/molecules/${encodeURIComponent(moleculeData.smiles)}/sdf`);
        if (resp.ok) {
          sdfBlock = await resp.text();
        }
      } catch (_) {}
    }
  }

  if (!sdfBlock || !sdfBlock.includes("M  END")) {
    // Si pas de SDF disponible, on recentre la caméra directement sur les résidus de la poche catalytique
    glViewer.zoomTo({ resi: info.pocketResidues }, 800);
    glViewer.render();
    return;
  }

  // Translation des coordonnées du ligand pour le centrer exactement dans la poche catalytique
  const translatedSdf = translateSdfCoordinates(sdfBlock, pocketCenter);

  // Retirer l'ancien ligand s'il existait
  if (currentLigandModel) {
    glViewer.removeModel(currentLigandModel);
  }

  try {
    currentLigandModel = glViewer.addModel(translatedSdf, "sdf");
    currentLigandModel.setStyle({}, {
      stick: { colorscheme: 'greenCarbon', radius: 0.28 },
      sphere: { scale: 0.22 }
    });

    // Recentrer la vue sur la molécule arrimée dans la poche
    glViewer.zoomTo({ model: currentLigandModel }, 800);
    glViewer.render();
  } catch (err) {
    console.error("Erreur affichage ligand 3D:", err);
  }
}

// Translation mathématique des atomes du fichier SDF vers le centre de la poche
function translateSdfCoordinates(sdfContent, targetCenter) {
  const lines = sdfContent.split('\n');
  const atomLines = [];
  let countsLineIndex = -1;

  for (let i = 0; i < lines.length; i++) {
    if (lines[i].includes("V2000")) {
      countsLineIndex = i;
      break;
    }
  }

  if (countsLineIndex === -1) return sdfContent;

  const atomCount = parseInt(lines[countsLineIndex].substring(0, 3).trim(), 10) || 0;
  if (atomCount === 0) return sdfContent;

  let sumX = 0, sumY = 0, sumZ = 0;
  const startIdx = countsLineIndex + 1;
  const endIdx = startIdx + atomCount;

  for (let i = startIdx; i < endIdx && i < lines.length; i++) {
    const l = lines[i];
    if (l.length >= 31) {
      const x = parseFloat(l.substring(0, 10));
      const y = parseFloat(l.substring(10, 20));
      const z = parseFloat(l.substring(20, 30));
      sumX += x;
      sumY += y;
      sumZ += z;
    }
  }

  const origCenterX = sumX / atomCount;
  const origCenterY = sumY / atomCount;
  const origCenterZ = sumZ / atomCount;

  const dx = targetCenter.x - origCenterX;
  const dy = targetCenter.y - origCenterY;
  const dz = targetCenter.z - origCenterZ;

  const newLines = [...lines];
  for (let i = startIdx; i < endIdx && i < lines.length; i++) {
    const l = lines[i];
    if (l.length >= 31) {
      const x = parseFloat(l.substring(0, 10)) + dx;
      const y = parseFloat(l.substring(10, 20)) + dy;
      const z = parseFloat(l.substring(20, 30)) + dz;
      const rest = l.substring(30);

      const xStr = x.toFixed(4).padStart(10, ' ');
      const yStr = y.toFixed(4).padStart(10, ' ');
      const zStr = z.toFixed(4).padStart(10, ' ');

      newLines[i] = `${xStr}${yStr}${zStr}${rest}`;
    }
  }

  return newLines.join('\n');
}

// Boutons de la barre d'outils 3D
if (viewPocketBtn) {
  viewPocketBtn.addEventListener('click', () => {
    const targetKey = diseaseSelect.value;
    const info = TARGET_DETAILS[targetKey] || TARGET_DETAILS["EGFR_T790M"];
    if (glViewer && info && info.pocketResidues) {
      if (currentLigandModel) {
        // Centrer sur le ligand arrimé et les résidus de la poche
        glViewer.zoomTo({
          or: [
            { model: currentLigandModel },
            { resi: info.pocketResidues }
          ]
        }, 700);
      } else {
        // Zoom direct précis sur les résidus physiques de la poche active
        glViewer.zoomTo({ resi: info.pocketResidues }, 700);
      }
      glViewer.render();
    }
  });
}

if (viewGlobalBtn) {
  viewGlobalBtn.addEventListener('click', () => {
    if (glViewer) glViewer.zoomTo(null, 700);
  });
}

if (toggleSurfaceBtn) {
  toggleSurfaceBtn.addEventListener('click', () => {
    showPocketSurface = !showPocketSurface;
    toggleSurfaceBtn.classList.toggle('active', showPocketSurface);
    const targetKey = diseaseSelect.value;
    const info = TARGET_DETAILS[targetKey] || TARGET_DETAILS["EGFR_T790M"];
    highlightPocket(info);
    if (glViewer) glViewer.render();
  });
}

if (toggleSpinBtn) {
  toggleSpinBtn.addEventListener('click', () => {
    isSpinning = !isSpinning;
    toggleSpinBtn.classList.toggle('active', isSpinning);
    if (glViewer) glViewer.spin(isSpinning);
  });
}

// ---------------------------------------------------------------------------
// 2. VISUALISATION 2D NATIVE AVEC SmilesDrawer DANS LE PANNEAU DE DROITE
// ---------------------------------------------------------------------------
function initSmilesDrawer() {
  if (window.SmilesDrawer && !smilesDrawer) {
    try {
      smilesDrawer = new window.SmilesDrawer.Drawer({
        width: 380,
        height: 170,
        bondThickness: 1.5,
        bondLength: 17,
        fontSizeLarge: 7,
        fontSizeSmall: 5,
        themes: {
          dark: {
            C: '#f3f4f6',
            O: '#f87171',
            N: '#60a5fa',
            F: '#34d399',
            CL: '#34d399',
            BR: '#fb923c',
            I: '#c084fc',
            P: '#f59e0b',
            S: '#fbbf24',
            B: '#f472b6',
            H: '#9ca3af',
            BACKGROUND: '#101116'
          }
        }
      });
    } catch (e) {
      console.warn("Initialisation SmilesDrawer:", e);
    }
  }
}

function render2DMolecule(moleculeData) {
  initSmilesDrawer();
  if (!moleculeData || !moleculeData.smiles) return;

  currentDisplayedSmiles = moleculeData.smiles;

  // Mise à jour des informations textuelles
  if (molPlaceholder) molPlaceholder.style.display = 'none';
  if (mol2dCanvas) mol2dCanvas.style.display = 'block';

  if (molSmilesText) molSmilesText.innerText = moleculeData.smiles;
  if (molMetaPkd) molMetaPkd.innerText = moleculeData.score ? `${Number(moleculeData.score).toFixed(2)} ★` : '—';
  if (molMetaMw) molMetaMw.innerText = moleculeData.mw ? `${Number(moleculeData.mw).toFixed(1)} g/mol` : '—';
  if (molMetaLogp) molMetaLogp.innerText = moleculeData.logp ? `${Number(moleculeData.logp).toFixed(2)}` : '—';
  if (molMetaSascore) molMetaSascore.innerText = moleculeData.sascore ? `${Number(moleculeData.sascore).toFixed(2)}` : '—';

  if (molStatusBadge) {
    if (moleculeData.is_hit || (moleculeData.score && moleculeData.score >= 8.5)) {
      molStatusBadge.innerText = '★ PÉPITE QUALIFIÉE';
      molStatusBadge.style.color = '#facc15';
    } else {
      molStatusBadge.innerText = 'Candidat évalué';
      molStatusBadge.style.color = '#10b981';
    }
  }

  // Rendu graphique 2D
  if (window.SmilesDrawer && smilesDrawer && mol2dCanvas) {
    try {
      window.SmilesDrawer.parse(moleculeData.smiles, function (tree) {
        smilesDrawer.draw(tree, mol2dCanvas, 'dark', false);
      }, function (err) {
        console.warn("Erreur dessin 2D SMILES:", err);
      });
    } catch (e) {
      console.warn("Exception dessin 2D:", e);
    }
  }
}

if (copySmiBtn) {
  copySmiBtn.addEventListener('click', () => {
    if (currentDisplayedSmiles) {
      navigator.clipboard.writeText(currentDisplayedSmiles);
      copySmiBtn.innerText = 'Copié !';
      setTimeout(() => { copySmiBtn.innerText = 'Copier'; }, 1500);
    }
  });
}

// ---------------------------------------------------------------------------
// 3. CONTRÔLE D'ALLOCATION DES RESSOURCES (CURSEUR CPU & ÉCONOMISEUR D'ÉCRAN)
// ---------------------------------------------------------------------------
function updateCpuAllocation(value) {
  currentCpuAllocation = parseInt(value, 10);
  if (cpuSlider) cpuSlider.value = currentCpuAllocation;
  if (cpuSliderVal) cpuSliderVal.innerText = `${currentCpuAllocation}% CPU`;

  presetButtons.forEach(btn => {
    btn.classList.toggle('active', parseInt(btn.dataset.cpu, 10) === currentCpuAllocation);
  });

  updateResourceStatus();

  // Transmission de l'allocation au backend Tauri
  try {
    invoke('set_resource_allocation', {
      cpuPercentage: currentCpuAllocation,
      idleOnly: isIdleOnly
    });
  } catch (_) {}
}

function updateResourceStatus() {
  if (!resourceStatusPill) return;

  if (isIdleOnly && !isUserIdle) {
    resourceStatusPill.innerText = "⏸️ En veille (Activité souris/clavier — Calcul en pause)";
    resourceStatusPill.className = "resource-status-pill paused";
  } else {
    resourceStatusPill.innerText = `● Calcul actif (${currentCpuAllocation}% CPU alloué)`;
    resourceStatusPill.className = "resource-status-pill";
  }
}

if (cpuSlider) {
  cpuSlider.addEventListener('input', (e) => {
    updateCpuAllocation(e.target.value);
  });
}

presetButtons.forEach(btn => {
  btn.addEventListener('click', () => {
    updateCpuAllocation(btn.dataset.cpu);
  });
});

if (idleOnlyToggle) {
  idleOnlyToggle.addEventListener('change', (e) => {
    isIdleOnly = e.target.checked;
    updateCpuAllocation(currentCpuAllocation);
  });
}

// Détection de l'inactivité de l'utilisateur (Mode économiseur d'écran)
function registerUserActivity() {
  lastActivityTime = Date.now();
  if (isUserIdle) {
    isUserIdle = false;
    updateResourceStatus();
    try {
      invoke('set_system_idle_state', { isIdle: false });
    } catch (_) {}
  }
}

['mousemove', 'keydown', 'mousedown', 'touchstart', 'scroll'].forEach(evt => {
  window.addEventListener(evt, registerUserActivity, { passive: true });
});

setInterval(() => {
  const idleElapsed = Date.now() - lastActivityTime;
  // Si inactif depuis plus de 20 secondes, on active l'état idle
  if (idleElapsed > 20000 && !isUserIdle) {
    isUserIdle = true;
    updateResourceStatus();
    try {
      invoke('set_system_idle_state', { isIdle: true });
    } catch (_) {}
  }
}, 2000);

// ---------------------------------------------------------------------------
// 4. LEADERBOARD EN TEMPS RÉEL (/leaderboard)
// ---------------------------------------------------------------------------
async function fetchLeaderboard() {
  try {
    const targetFilter = lbFilterTarget ? lbFilterTarget.value : '';
    let data = null;

    // 1. Tenter via le backend natif Rust (immunisé contre ATS macOS et CORS)
    try {
      data = await invoke('fetch_leaderboard', {
        target: targetFilter || null,
        url: SERVER_URL || null
      });
    } catch (invokeErr) {
      // 2. Repli éventuel sur fetch
      const query = targetFilter ? `?target=${encodeURIComponent(targetFilter)}` : '';
      const primaryUrl = `${SERVER_URL}/leaderboard${query}`;
      let res = null;
      try {
        res = await fetch(primaryUrl);
      } catch (netErr) {
        const fallbackUrl = SERVER_URL.includes("srv1091727.hstgr.cloud")
          ? `http://127.0.0.1:8000/leaderboard${query}`
          : `https://srv1091727.hstgr.cloud/leaderboard${query}`;
        try {
          res = await fetch(fallbackUrl);
        } catch (_) {
          throw new Error(invokeErr);
        }
      }
      if (!res || !res.ok) throw new Error(res ? `HTTP ${res.status}` : "Serveur injoignable");
      data = await res.json();
    }

    // Statistiques globales du bandeau
    if (lbStatScreened) lbStatScreened.innerText = (data.total_molecules_screened || 0).toLocaleString();
    if (lbStatHits) lbStatHits.innerText = (data.total_hits || 0).toLocaleString();
    if (lbStatBest) lbStatBest.innerText = data.best_pkd_score ? data.best_pkd_score.toFixed(2) : '—';
    if (lbStatWorkers) lbStatWorkers.innerText = data.active_workers || 1;

    renderLeaderboardHits(data.top_hits || []);
  } catch (err) {
    if (lbHitsList) {
      lbHitsList.innerHTML = `<div style="text-align: center; color: #f87171; padding: 20px;">Impossible de contacter le serveur central (${err.message})<br><small style="color: var(--text-dim); margin-top: 6px; display: inline-block;">Serveur cible : ${SERVER_URL}</small></div>`;
    }
  }
}

function renderLeaderboardHits(hits) {
  if (!lbHitsList) return;

  if (hits.length === 0) {
    lbHitsList.innerHTML = `<div style="text-align: center; color: var(--text-dim); padding: 30px;">Aucune pépite enregistrée pour cette cible pour le moment.</div>`;
    return;
  }

  lbHitsList.innerHTML = '';
  hits.forEach((hit, index) => {
    const card = document.createElement('div');
    const rank = index + 1;
    card.className = `hit-card ${rank <= 3 ? `rank-${rank}` : ''}`;

    let medal = `#${rank}`;
    if (rank === 1) medal = '🥇';
    else if (rank === 2) medal = '🥈';
    else if (rank === 3) medal = '🥉';

    const byStr = hit.discovered_by || 'Bénévole Anonyme';
    const targetInfo = TARGET_DETAILS[hit.target] || { name: hit.target || 'EGFR' };

    card.innerHTML = `
      <div class="hit-rank">${medal}</div>
      <div class="hit-info">
        <div class="hit-title">
          <span>${targetInfo.name}</span>
          <span class="hit-pkd-badge">pKd: ${Number(hit.score).toFixed(2)}</span>
        </div>
        <div class="hit-smiles" title="${hit.smiles}">${hit.smiles}</div>
        <div class="hit-by">Découvert par : <strong>${byStr}</strong></div>
      </div>
      <div class="hit-actions">
        <button class="btn-secondary inspect-hit-btn" title="Inspecter en 2D & 3D">🔬 Voir</button>
        <button class="btn-secondary download-sdf-btn" title="Télécharger la conformation 3D SDF">📥</button>
      </div>
    `;

    // Bouton Inspecter en 2D et 3D
    card.querySelector('.inspect-hit-btn').addEventListener('click', () => {
      // Basculer sur l'onglet de calcul pour voir la 2D et 3D
      document.querySelector('[data-tab="tab-compute"]').click();

      // Ajuster la cible si différente
      if (hit.target && diseaseSelect.value !== hit.target) {
        diseaseSelect.value = hit.target;
        handleTargetChange();
      }

      // Rendu 2D et 3D
      render2DMolecule(hit);
      displayLigandInPocket(hit);
      showToast(`Pépite chargée : pKd ${Number(hit.score).toFixed(2)} (${byStr})`);
    });

    // Bouton Télécharger SDF
    card.querySelector('.download-sdf-btn').addEventListener('click', () => {
      window.open(`${SERVER_URL}/molecules/${hit.id || encodeURIComponent(hit.smiles)}/sdf`, '_blank');
    });

    lbHitsList.appendChild(card);
  });
}

if (lbRefreshBtn) lbRefreshBtn.addEventListener('click', fetchLeaderboard);
if (lbFilterTarget) lbFilterTarget.addEventListener('change', fetchLeaderboard);

if (lbExportSdfBtn) {
  lbExportSdfBtn.addEventListener('click', () => {
    const target = lbFilterTarget ? lbFilterTarget.value : '';
    const url = target
      ? `${SERVER_URL}/leaderboard/export/sdf?target=${target}`
      : `${SERVER_URL}/leaderboard/export/sdf`;
    window.open(url, '_blank');
  });
}

// ---------------------------------------------------------------------------
// 5. GAMIFICATION, PROFILS BÉNÉVOLES & STATISTIQUES D'IMPACT
// ---------------------------------------------------------------------------
async function saveVolunteerPseudo(val) {
  const trimmed = (val || '').trim();
  volunteerPseudo = trimmed;
  localStorage.setItem('helixgrid_volunteer_pseudo', trimmed);
  if (volunteerPseudoInput) volunteerPseudoInput.value = trimmed;
  if (settingsPseudoInput) settingsPseudoInput.value = trimmed;

  try {
    await invoke('set_volunteer_pseudo', { pseudo: trimmed });
    showToast(`Identifiant enregistré : ${trimmed || 'Anonyme'}`);
  } catch (err) {
    showToast(`Erreur d'enregistrement : ${err}`, 'error');
  }
}

function initVolunteerProfile() {
  if (volunteerPseudoInput && volunteerPseudo) {
    volunteerPseudoInput.value = volunteerPseudo;
  }
  if (settingsPseudoInput && volunteerPseudo) {
    settingsPseudoInput.value = volunteerPseudo;
  }

  updateProfileAndBadgesUI();
}

if (savePseudoBtn) {
  savePseudoBtn.addEventListener('click', () => {
    saveVolunteerPseudo(volunteerPseudoInput ? volunteerPseudoInput.value : '');
  });
}

function recordEvaluationMetrics(metrics) {
  const screenedCount = metrics.screened_batch || 0;
  userTotalScreened += screenedCount;
  localStorage.setItem('helixgrid_user_screened', userTotalScreened.toString());

  if (metrics.total_hits_local) {
    userTotalHits = metrics.total_hits_local;
    localStorage.setItem('helixgrid_user_hits', userTotalHits.toString());
  }

  // Ventilation de la recherche par pathologie
  const targetKey = metrics.target || diseaseSelect.value;
  const info = TARGET_DETAILS[targetKey];
  const cat = info ? info.diseaseCategory : 'cancer';
  diseaseEfforts[cat] = (diseaseEfforts[cat] || 0) + screenedCount;
  localStorage.setItem('helixgrid_disease_efforts', JSON.stringify(diseaseEfforts));

  updateProfileAndBadgesUI();
  checkBadges();
}

function updateProfileAndBadgesUI() {
  if (userStatScreened) userStatScreened.innerText = userTotalScreened.toLocaleString();
  if (userStatHits) userStatHits.innerText = userTotalHits.toString();

  // Temps de calcul
  const mins = Math.floor(userComputeSeconds / 60);
  const hrs = Math.floor(mins / 60);
  if (userStatTime) {
    userStatTime.innerText = hrs > 0 ? `${hrs}h ${mins % 60}m` : `${mins}m`;
  }

  // Énergie informatique estimée (TDP moyen ~65W multiplié par le duty cycle)
  const kwh = (userComputeSeconds / 3600) * 0.065 * (currentCpuAllocation / 100);
  if (userStatEnergy) {
    userStatEnergy.innerText = `${kwh.toFixed(3)} kWh`;
  }

  // Barres de répartition de la recherche par pathologie
  const totalEffort = (diseaseEfforts.cancer || 0) + (diseaseEfforts.neuro || 0) + (diseaseEfforts.antiviral || 0);
  if (totalEffort > 0) {
    const pctCancer = Math.round((diseaseEfforts.cancer / totalEffort) * 100);
    const pctNeuro = Math.round((diseaseEfforts.neuro / totalEffort) * 100);
    const pctAntiviral = Math.round((diseaseEfforts.antiviral / totalEffort) * 100);

    if (impactBarCancer) impactBarCancer.style.width = `${pctCancer}%`;
    if (impactPctCancer) impactPctCancer.innerText = `${pctCancer}% (${diseaseEfforts.cancer.toLocaleString()} mol)`;

    if (impactBarNeuro) impactBarNeuro.style.width = `${pctNeuro}%`;
    if (impactPctNeuro) impactPctNeuro.innerText = `${pctNeuro}% (${diseaseEfforts.neuro.toLocaleString()} mol)`;

    if (impactBarAntiviral) impactBarAntiviral.style.width = `${pctAntiviral}%`;
    if (impactPctAntiviral) impactPctAntiviral.innerText = `${pctAntiviral}% (${diseaseEfforts.antiviral.toLocaleString()} mol)`;
  }

  // Rang / Tier citoyen
  if (volunteerTierBadge) {
    if (userTotalScreened >= 2500) volunteerTierBadge.innerText = '🔬 Maître Découvreur';
    else if (userTotalScreened >= 1000) volunteerTierBadge.innerText = '⚡ Architecte Moléculaire';
    else if (userTotalScreened >= 200) volunteerTierBadge.innerText = '🧪 Cribleur Distribué';
    else volunteerTierBadge.innerText = '🌟 Chercheur Citoyen';
  }

  // Mise à jour de l'état visuel des 8 badges
  const badgeMap = [
    { id: 'badge-first-step', unlocked: userTotalScreened >= 50 },
    { id: 'badge-distributed-power', unlocked: userTotalScreened >= 500 },
    { id: 'badge-supercomputer', unlocked: userTotalScreened >= 2500 },
    { id: 'badge-subnanomolar', unlocked: unlockedBadges.has('badge-subnanomolar') },
    { id: 'badge-oncogene', unlocked: (diseaseEfforts.cancer || 0) >= 100 && userTotalHits >= 1 },
    { id: 'badge-neuro', unlocked: (diseaseEfforts.neuro || 0) >= 100 && userTotalHits >= 1 },
    { id: 'badge-medicinal', unlocked: unlockedBadges.has('badge-medicinal') },
    { id: 'badge-top-contributor', unlocked: userTotalHits >= 2 || userTotalScreened >= 1000 }
  ];

  let unlockedCount = 0;
  badgeMap.forEach(b => {
    const el = document.getElementById(b.id);
    if (el) {
      if (b.unlocked) {
        el.classList.add('unlocked');
        const st = el.querySelector('.badge-status');
        if (st) st.innerText = '🔓 Débloqué';
        unlockedCount++;
      }
    }
  });

  if (badgesUnlockedCount) {
    badgesUnlockedCount.innerText = `${unlockedCount} / 8 débloqués`;
  }
}

function checkBadges() {
  const candidatesToCheck = [
    { id: 'badge-first-step', name: 'Premier Pas Scientifique', cond: userTotalScreened >= 50 },
    { id: 'badge-distributed-power', name: 'Puissance Distribuée', cond: userTotalScreened >= 500 },
    { id: 'badge-supercomputer', name: 'Super-Calculateur Citoyen', cond: userTotalScreened >= 2500 },
    { id: 'badge-oncogene', name: 'Traqueur d\'Oncogène', cond: (diseaseEfforts.cancer || 0) >= 100 && userTotalHits >= 1 },
    { id: 'badge-neuro', name: 'Pionnier Neuro-Protecteur', cond: (diseaseEfforts.neuro || 0) >= 100 && userTotalHits >= 1 },
    { id: 'badge-top-contributor', name: 'Héros d\'HelixGrid', cond: userTotalHits >= 2 || userTotalScreened >= 1000 }
  ];

  candidatesToCheck.forEach(b => {
    if (b.cond && !unlockedBadges.has(b.id)) {
      unlockedBadges.add(b.id);
      localStorage.setItem('helixgrid_unlocked_badges', JSON.stringify([...unlockedBadges]));
      showToast(`🏆 Badge Débloqué : ${b.name} !`, 'achievement');
    }
  });
}

// Timer régulier du temps de calcul (quand le calcul tourne)
setInterval(() => {
  if (workerStarted && (!isIdleOnly || isUserIdle)) {
    userComputeSeconds += 1;
    if (userComputeSeconds % 10 === 0) {
      localStorage.setItem('helixgrid_user_seconds', userComputeSeconds.toString());
      updateProfileAndBadgesUI();
    }
  }
}, 1000);

// ---------------------------------------------------------------------------
// 6. GESTION DES CIBLES & LOGS
// ---------------------------------------------------------------------------
function appendLog(text) {
  const line = document.createElement('div');
  line.className = 'log-line';
  const str = String(text ?? '');

  if (str.startsWith('ERROR:') || str.includes('Erreur') || str.includes('[!]')) {
    line.classList.add('error');
  } else if (str.includes('★ PÉPITE') || str.includes('NOUVELLE PÉPITE') || str.includes('Rang mondial')) {
    line.classList.add('hit');
  }

  line.innerText = str;
  logContainer.appendChild(line);
  logContainer.scrollTop = logContainer.scrollHeight;
}

function handleTargetChange() {
  const selectedKey = diseaseSelect.value;
  const info = TARGET_DETAILS[selectedKey] || TARGET_DETAILS["EGFR_T790M"];

  targetTitleEl.innerText = info.name;
  pdbBadgeEl.innerText = `PDB: ${info.pdb}`;
  targetDescBox.innerText = info.desc;

  loadPdbInViewer(info.pdb);
}

diseaseSelect.addEventListener('change', handleTargetChange);

// ---------------------------------------------------------------------------
// 7. AUTO-UPDATE
// ---------------------------------------------------------------------------
async function checkForUpdates() {
  try {
    const updateInfo = await invoke('check_update');
    if (updateInfo.available) {
      appendLog(`⚠️ Nouvelle version disponible : ${updateInfo.version}`);
      if (updateBanner) updateBanner.style.display = 'block';
    }
  } catch (_) {}
}

if (updateBtn) {
  updateBtn.addEventListener('click', async () => {
    updateBtn.disabled = true;
    updateBtn.innerText = "Mise à jour en cours...";
    try {
      await invoke('install_update');
    } catch (e) {
      appendLog(`❌ Erreur màj : ${e}`);
    }
  });
}

// ---------------------------------------------------------------------------
// 8. LOGIQUE DE LA PAGE PARAMÈTRES
// ---------------------------------------------------------------------------
async function loadAccountSettings() {
  try {
    const info = await invoke('get_account_settings');
    if (settingPreventSleep) settingPreventSleep.checked = !!info.prevent_sleep;
    if (settingKeepTray) settingKeepTray.checked = !!info.keep_in_menu_bar;
    if (settingsPseudoInput) settingsPseudoInput.value = info.pseudo || volunteerPseudo || '';
    if (settingsPubkeyDisplay) {
      settingsPubkeyDisplay.innerText = info.public_key || 'Clé non initialisée';
      settingsPubkeyDisplay.title = info.public_key || '';
    }
    if (settingsBufferStatus) {
      const count = info.pending_submissions || 0;
      settingsBufferStatus.innerText = count === 0
        ? "0 pépite en attente (Toutes synchronisées)"
        : `${count} pépite(s) tamponnée(s) en attente d'envoi`;
    }
    if (settingsServerUrlInput) {
      settingsServerUrlInput.value = info.server_url || localStorage.getItem('helixgrid_server_url') || DEFAULT_REMOTE_SERVER;
    }
    if (info.is_running && !workerStarted) {
      workerStarted = true;
      startBtn.innerText = "⏹️ Arrêter le Calcul";
      startBtn.classList.add('btn-stop');
      diseaseSelect.disabled = true;
      engineSelect.disabled = true;
      resourceStatusPill.innerText = `● Calcul actif (${currentCpuAllocation}% CPU alloué)`;
      resourceStatusPill.className = "resource-status-pill";
    }
  } catch (err) {
    console.error("Erreur chargement paramètres :", err);
  }
}

if (settingPreventSleep) {
  settingPreventSleep.addEventListener('change', async (e) => {
    const enable = e.target.checked;
    try {
      await invoke('set_prevent_sleep', { enable });
      showToast(enable ? "⚡ Mise en veille désactivée pendant le calcul" : "💤 Mode veille normal réactivé");
    } catch (err) {
      showToast(`Erreur mise en veille : ${err}`, 'error');
    }
  });
}

if (settingKeepTray) {
  settingKeepTray.addEventListener('change', async (e) => {
    const enable = e.target.checked;
    try {
      await invoke('set_keep_in_menu_bar', { enable });
      showToast(enable ? "📌 Actif en arrière-plan dans la barre des menus à la fermeture" : "❌ Quittera complètement à la fermeture");
    } catch (err) {
      showToast(`Erreur barre des menus : ${err}`, 'error');
    }
  });
}

if (settingsSavePseudoBtn) {
  settingsSavePseudoBtn.addEventListener('click', () => {
    saveVolunteerPseudo(settingsPseudoInput ? settingsPseudoInput.value : '');
  });
}

if (settingsCopyPubkeyBtn) {
  settingsCopyPubkeyBtn.addEventListener('click', async () => {
    const key = settingsPubkeyDisplay ? settingsPubkeyDisplay.innerText : '';
    if (key && key !== 'Chargement de la clé...' && key !== 'Clé non initialisée') {
      try {
        await navigator.clipboard.writeText(key);
        showToast("📋 Clé publique Ed25519 copiée !");
      } catch (_) {
        showToast("Impossible de copier la clé.", "error");
      }
    }
  });
}

if (settingsOpenDirBtn) {
  settingsOpenDirBtn.addEventListener('click', async () => {
    try {
      await invoke('open_storage_dir');
      showToast("📂 Dossier de stockage ouvert");
    } catch (err) {
      showToast(`Erreur ouverture dossier : ${err}`, 'error');
    }
  });
}

if (settingsTestServerBtn) {
  settingsTestServerBtn.addEventListener('click', async () => {
    const url = (settingsServerUrlInput ? settingsServerUrlInput.value.trim() : '') || DEFAULT_REMOTE_SERVER;
    if (settingsServerStatus) {
      settingsServerStatus.innerText = "⏳ Test de connexion en cours...";
      settingsServerStatus.style.color = "var(--text-muted)";
    }
    try {
      const res = await invoke('test_server_connection', { url: url || null });
      if (res.ok) {
        if (settingsServerStatus) {
          settingsServerStatus.innerText = `🟢 Connecté (${res.latency_ms} ms — HTTP ${res.status})`;
          settingsServerStatus.style.color = "var(--accent-green)";
        }
        showToast(`🟢 Serveur joignable en ${res.latency_ms} ms`);
      } else {
        if (settingsServerStatus) {
          settingsServerStatus.innerText = `🔴 ${res.message}`;
          settingsServerStatus.style.color = "var(--accent-red)";
        }
        showToast(`🔴 ${res.message}`, "error");
      }
    } catch (err) {
      if (settingsServerStatus) {
        settingsServerStatus.innerText = `🔴 Échec : ${err}`;
        settingsServerStatus.style.color = "var(--accent-red)";
      }
      showToast(`🔴 Erreur test : ${err}`, "error");
    }
  });
}

if (settingsSaveServerBtn) {
  settingsSaveServerBtn.addEventListener('click', async () => {
    const url = (settingsServerUrlInput ? settingsServerUrlInput.value.trim() : '') || DEFAULT_REMOTE_SERVER;
    SERVER_URL = url;
    localStorage.setItem('helixgrid_server_url', url);
    try {
      await invoke('set_server_url', { url });
      showToast("🌐 Adresse serveur mise à jour");
      fetchLeaderboard();
    } catch (e) {
      showToast(`Erreur serveur : ${e}`, 'error');
    }
  });
}

if (settingsResetServerBtn) {
  settingsResetServerBtn.addEventListener('click', async () => {
    SERVER_URL = DEFAULT_REMOTE_SERVER;
    localStorage.removeItem('helixgrid_server_url');
    if (settingsServerUrlInput) settingsServerUrlInput.value = DEFAULT_REMOTE_SERVER;
    try {
      await invoke('set_server_url', { url: DEFAULT_REMOTE_SERVER });
      showToast("🔄 Adresse officielle rétablie");
      if (settingsServerStatus) {
        settingsServerStatus.innerText = "État : Prêt";
        settingsServerStatus.style.color = "var(--text-dim)";
      }
      fetchLeaderboard();
    } catch (e) {
      showToast(`Erreur : ${e}`, 'error');
    }
  });
}

// ---------------------------------------------------------------------------
// 9. DÉMARRAGE / ARRÊT DU CALCUL ET ÉCOUTE DES ÉVÉNEMENTS TAURI
// ---------------------------------------------------------------------------
startBtn.addEventListener('click', async () => {
  if (workerStarted) {
    // Demande d'arrêt par l'utilisateur
    startBtn.innerText = "⏳ Arrêt en cours...";
    startBtn.disabled = true;
    appendLog("[⏹] Demande d'arrêt envoyée au moteur de calcul...");
    try {
      await invoke('stop_worker');
    } catch (err) {
      appendLog(`[!] Erreur lors de l'arrêt : ${err}`);
      startBtn.disabled = false;
    }
    return;
  }

  // Démarrage du worker
  const selectedTarget = diseaseSelect.value;
  const selectedEngine = engineSelect.value;
  const targetInfo = TARGET_DETAILS[selectedTarget] || { name: selectedTarget };

  workerStarted = true;
  startBtn.innerText = "⏹️ Arrêter le Calcul";
  startBtn.classList.add('btn-stop');
  startBtn.disabled = false;
  diseaseSelect.disabled = true;
  engineSelect.disabled = true;

  try {
    appendLog("=".repeat(50));
    appendLog(`🚀 Démarrage de la recherche : ${targetInfo.name}`);
    appendLog(`🧠 Moteur sélectionné : ${engineSelect.options[engineSelect.selectedIndex].text}`);
    appendLog(`⚡ Allocation CPU : ${currentCpuAllocation}% (Duty-cycle adaptatif)`);
    appendLog("=".repeat(50));

    await invoke('start_worker', {
      target: selectedTarget,
      engine: selectedEngine,
      pseudo: volunteerPseudo || null
    });

    appendLog("[✓] Worker autonome lancé avec succès !");
  } catch (error) {
    appendLog(`Erreur de lancement : ${error}`);
    workerStarted = false;
    startBtn.innerText = "🚀 Démarrer la Génération & le Calcul";
    startBtn.classList.remove('btn-stop');
    startBtn.disabled = false;
    diseaseSelect.disabled = false;
    engineSelect.disabled = false;
  }
});

async function setupBackendListeners() {
  // 1. Écoute des logs émis par le backend
  await listen('worker-log', (event) => {
    appendLog(event.payload);
  });

  // 2. Écoute de chaque molécule découverte pour affichage 2D et 3D en direct
  await listen('molecule-discovered', (event) => {
    const mol = event.payload;
    render2DMolecule(mol);

    // Si c'est une pépite qualifiée, vérifier les badges spéciaux
    if (mol.score >= 9.0) {
      if (!unlockedBadges.has('badge-subnanomolar')) {
        unlockedBadges.add('badge-subnanomolar');
        localStorage.setItem('helixgrid_unlocked_badges', JSON.stringify([...unlockedBadges]));
        showToast('💎 Badge Débloqué : Inhibiteur Sub-Nanomolaire (pKd ≥ 9.0) !', 'achievement');
      }
    }
    if (mol.pains_ok && mol.brenk_ok && mol.sascore <= 3.0) {
      if (!unlockedBadges.has('badge-medicinal')) {
        unlockedBadges.add('badge-medicinal');
        localStorage.setItem('helixgrid_unlocked_badges', JSON.stringify([...unlockedBadges]));
        showToast('🛡️ Badge Débloqué : Chimiste Médicinal Pur !', 'achievement');
      }
    }

    // Affichage 3D du ligand dans la poche
    displayLigandInPocket(mol);
  });

  // 3. Écoute des métriques d'évaluation pour la gamification
  await listen('worker-metrics', (event) => {
    recordEvaluationMetrics(event.payload);
  });

  // 4. Écoute de l'état de pause de l'économiseur d'écran
  await listen('worker-idle-paused', (event) => {
    const isPaused = event.payload;
    if (isPaused) {
      resourceStatusPill.innerText = "⏸️ En veille (Activité utilisateur détectée — Calcul en pause)";
      resourceStatusPill.className = "resource-status-pill paused";
    } else {
      resourceStatusPill.innerText = `● Calcul actif (${currentCpuAllocation}% CPU alloué)`;
      resourceStatusPill.className = "resource-status-pill";
    }
  });

  // 5. Écoute de l'arrêt du worker
  await listen('worker-stopped', () => {
    workerStarted = false;
    startBtn.innerText = "🚀 Démarrer la Génération & le Calcul";
    startBtn.classList.remove('btn-stop');
    startBtn.disabled = false;
    diseaseSelect.disabled = false;
    engineSelect.disabled = false;
    resourceStatusPill.innerText = "⏸️ Calcul en pause (Arrêté)";
    resourceStatusPill.className = "resource-status-pill paused";
    appendLog("[✓] Worker arrêté.");
  });
}

// ---------------------------------------------------------------------------
// 10. INITIALISATIONS AU CHARGEMENT DE L'APPLICATION
// ---------------------------------------------------------------------------
handleTargetChange();
initSmilesDrawer();
initVolunteerProfile();
updateCpuAllocation(100);
setupBackendListeners();
loadAccountSettings();
checkForUpdates();
fetchLeaderboard();
