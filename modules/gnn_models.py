import os
import torch
import torch.nn as nn
import torch.optim as optim
import numpy as np
import pandas as pd
from typing import Optional, Dict
from rdkit import Chem
from transformers import AutoTokenizer, AutoModel

# ---------------------------------------------------------------------------
# 1. Directed Message Passing Neural Network (D-MPNN / Chemprop)
# ---------------------------------------------------------------------------

def mol_to_graph(mol):
    """
    Convertit un objet RDKit Mol en tenseurs de graphe pour le D-MPNN.
    Extrait les caractéristiques atomiques (30 dims) et de liaisons orientées (10 dims).
    """
    if mol is None:
        return None

    # Caractéristiques atomiques
    atom_features = []
    for atom in mol.GetAtoms():
        num = atom.GetAtomicNum()
        atomic_one_hot = [num == x for x in [6, 7, 8, 9, 15, 16, 17, 35, 53]]  # C, N, O, F, P, S, Cl, Br, I
        atomic_one_hot.append(not any(atomic_one_hot))  # Autres atomes
        deg = [atom.GetDegree() == x for x in range(6)]
        charge = [atom.GetFormalCharge() == x for x in [-1, 0, 1]]
        charge.append(not any(charge))
        hyb = [atom.GetHybridization() == x for x in [
            Chem.rdchem.HybridizationType.SP,
            Chem.rdchem.HybridizationType.SP2,
            Chem.rdchem.HybridizationType.SP3
        ]]
        hyb.append(not any(hyb))
        arom = [atom.GetIsAromatic()]
        num_h = [atom.GetTotalNumHs() == x for x in range(5)]
        feats = atomic_one_hot + deg + charge + hyb + arom + num_h
        atom_features.append([float(x) for x in feats])

    # Arêtes orientées (directed edges)
    edge_features = []
    edges = []
    reverse_edge_idx = {}
    atom_incoming_edges = {i: [] for i in range(mol.GetNumAtoms())}

    for bond in mol.GetBonds():
        u = bond.GetBeginAtomIdx()
        v = bond.GetEndAtomIdx()
        bt = bond.GetBondType()
        b_type = [
            bt == Chem.rdchem.BondType.SINGLE,
            bt == Chem.rdchem.BondType.DOUBLE,
            bt == Chem.rdchem.BondType.TRIPLE,
            bt == Chem.rdchem.BondType.AROMATIC
        ]
        conj = [bond.GetIsConjugated()]
        ring = [bond.IsInRing()]
        stereo = [bond.GetStereo() == x for x in [
            Chem.rdchem.BondStereo.STEREOZ,
            Chem.rdchem.BondStereo.STEREOE,
            Chem.rdchem.BondStereo.STEREOANY,
            Chem.rdchem.BondStereo.STEREONONE
        ]]
        b_feats = [float(x) for x in (b_type + conj + ring + stereo)]

        idx_uv = len(edges)
        edges.append((u, v))
        edge_features.append(b_feats)
        atom_incoming_edges[v].append(idx_uv)

        idx_vu = len(edges)
        edges.append((v, u))
        edge_features.append(b_feats)
        atom_incoming_edges[u].append(idx_vu)

        reverse_edge_idx[idx_uv] = idx_vu
        reverse_edge_idx[idx_vu] = idx_uv

    # Pour chaque liaison (u -> v), arêtes arrivant en u sauf le retour (v -> u)
    edge_incoming = []
    for e_idx, (u, v) in enumerate(edges):
        rev = reverse_edge_idx[e_idx]
        inc = [in_e for in_e in atom_incoming_edges[u] if in_e != rev]
        edge_incoming.append(inc)

    return {
        'num_atoms': mol.GetNumAtoms(),
        'atom_features': torch.tensor(atom_features, dtype=torch.float32),
        'edge_features': torch.tensor(edge_features, dtype=torch.float32) if edges else torch.zeros((0, 10)),
        'edges': edges,
        'edge_incoming': edge_incoming,
        'atom_incoming': atom_incoming_edges
    }


class DMPNN(nn.Module):
    """
    Directed Message Passing Neural Network (architecture de référence Chemprop).
    Apprend les représentations sur les liaisons dirigées pour éviter les boucles d'information.
    """
    def __init__(self, atom_dim=30, edge_dim=10, hidden_dim=128, depth=3):
        super(DMPNN, self).__init__()
        self.depth = depth
        self.hidden_dim = hidden_dim
        self.w_i = nn.Linear(atom_dim + edge_dim, hidden_dim)
        self.w_m = nn.Linear(hidden_dim, hidden_dim, bias=False)
        self.w_a = nn.Linear(atom_dim + hidden_dim, hidden_dim)
        self.act = nn.ReLU()
        self.readout = nn.Sequential(
            nn.Linear(hidden_dim * 2, hidden_dim),
            nn.ReLU(),
            nn.Dropout(0.15),
            nn.Linear(hidden_dim, 64),
            nn.ReLU(),
            nn.Linear(64, 1)
        )

    def forward(self, g):
        num_atoms = g['num_atoms']
        atom_feats = g['atom_features']
        edge_feats = g['edge_features']
        edges = g['edges']
        edge_incoming = g['edge_incoming']
        atom_incoming = g['atom_incoming']

        num_edges = len(edges)
        if num_edges == 0:
            h_v = self.act(self.w_a(torch.cat([atom_feats, torch.zeros(num_atoms, self.hidden_dim)], dim=1)))
            h_mean = h_v.mean(dim=0, keepdim=True)
            h_max = h_v.max(dim=0, keepdim=True)[0]
            return self.readout(torch.cat([h_mean, h_max], dim=1))

        u_indices = torch.tensor([e[0] for e in edges], dtype=torch.long)
        h_0 = self.act(self.w_i(torch.cat([atom_feats[u_indices], edge_feats], dim=1)))
        h = h_0

        for _ in range(self.depth):
            m = torch.zeros_like(h)
            for e_idx, inc in enumerate(edge_incoming):
                if inc:
                    m[e_idx] = h[inc].sum(dim=0)
            h = self.act(h_0 + self.w_m(m))

        atom_m = torch.zeros(num_atoms, h.size(1))
        for a_idx, inc in atom_incoming.items():
            if inc:
                atom_m[a_idx] = h[inc].sum(dim=0)

        h_v = self.act(self.w_a(torch.cat([atom_feats, atom_m], dim=1)))
        h_mean = h_v.mean(dim=0, keepdim=True)
        h_max = h_v.max(dim=0, keepdim=True)[0]
        h_mol = torch.cat([h_mean, h_max], dim=1)
        return self.readout(h_mol)


# ---------------------------------------------------------------------------
# 2. ChemBERTa Contextual Transformer Predictor
# ---------------------------------------------------------------------------

class ChemBERTaPredictor(nn.Module):
    """
    Modèle prédisant l'affinité pKd à partir des embeddings denses (384-D)
    extraits du modèle fondation ChemBERTa-77M.
    """
    def __init__(self, embed_dim=384, hidden_dim=128):
        super(ChemBERTaPredictor, self).__init__()
        self.head = nn.Sequential(
            nn.Linear(embed_dim, hidden_dim),
            nn.BatchNorm1d(hidden_dim),
            nn.ReLU(),
            nn.Dropout(0.2),
            nn.Linear(hidden_dim, 64),
            nn.ReLU(),
            nn.Linear(64, 1)
        )

    def forward(self, x):
        return self.head(x)


# ---------------------------------------------------------------------------
# 3. Pipelines de prédiction et gestion de modèles pré-entraînés
# ---------------------------------------------------------------------------

_CHEMBERTA_TOKENIZER = None
_CHEMBERTA_MODEL = None

def get_chemberta():
    global _CHEMBERTA_TOKENIZER, _CHEMBERTA_MODEL
    if _CHEMBERTA_MODEL is None:
        model_name = "DeepChem/ChemBERTa-77M-MLM"
        _CHEMBERTA_TOKENIZER = AutoTokenizer.from_pretrained(model_name)
        _CHEMBERTA_MODEL = AutoModel.from_pretrained(model_name)
        _CHEMBERTA_MODEL.eval()
    return _CHEMBERTA_TOKENIZER, _CHEMBERTA_MODEL


def get_chemberta_embedding(smiles: str) -> Optional[torch.Tensor]:
    try:
        tok, bert = get_chemberta()
        inputs = tok(smiles, return_tensors="pt", truncation=True, max_length=128)
        with torch.no_grad():
            outputs = bert(**inputs)
            # Embedding du token [CLS]
            cls_emb = outputs.last_hidden_state[:, 0, :]
        return cls_emb
    except Exception:
        return None
