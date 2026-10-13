import random
from typing import List, Optional
import selfies as sf
from rdkit import Chem

# Alphabet de tokens SELFIES pharmacophore et bioisostères ciblés
PHARMACOPHORE_TOKENS = [
    # Halogènes (interactions hydrophobes et halogène-bonding)
    "[F]", "[Cl]", "[Br]",
    # Hétéroatomes donneurs/accepteurs de liaisons hydrogène
    "[O]", "[N]", "[S]", "[=O]", "[=N]", "[NH1]",
    # Groupements alkyles et insaturations
    "[C]", "[=C]", "[#C]",
    # Branches & hétérocycles
    "[Branch1]", "[Ring1]", "[Branch2]", "[Ring2]"
]

BIOISOSTERE_REPLACEMENTS = {
    "[F]": ["[Cl]", "[C]", "[O]"],
    "[Cl]": ["[F]", "[Br]", "[C]"],
    "[O]": ["[S]", "[N]", "[C]"],
    "[S]": ["[O]", "[C]"],
    "[N]": ["[O]", "[C]"],
    "[C]": ["[N]", "[O]", "[F]"]
}


def smiles_to_selfies(smiles: str) -> Optional[str]:
    """Convertit un SMILES en chaîne SELFIES 100% robuste."""
    try:
        mol = Chem.MolFromSmiles(smiles)
        if mol is None:
            return None
        # Normalisation canonique via RDKit avant encodage
        can_smiles = Chem.MolToSmiles(mol, canonical=True)
        return sf.encoder(can_smiles)
    except Exception:
        return None


def selfies_to_smiles(selfies_str: str) -> Optional[str]:
    """Convertit une chaîne SELFIES en SMILES canonique validé par RDKit."""
    try:
        smi = sf.decoder(selfies_str)
        if not smi:
            return None
        mol = Chem.MolFromSmiles(smi)
        if mol is None:
            return None
        return Chem.MolToSmiles(mol, canonical=True)
    except Exception:
        return None


def mutate_selfies(selfies_str: str, max_mutations: int = 2) -> Optional[str]:
    """
    Applique une ou plusieurs mutations stochastiques sur une chaîne SELFIES :
    - Remplacement de token (bioisostère ou alphabet ciblé)
    - Insertion d'un token (croissance d'élongation ou pharmacophore)
    - Suppression d'un token (simplification moléculaire)
    """
    try:
        tokens = list(sf.split_selfies(selfies_str))
        if not tokens:
            return None

        num_mutations = random.randint(1, max_mutations)
        new_tokens = list(tokens)

        for _ in range(num_mutations):
            op = random.choices(["substitute", "insert", "delete"], weights=[0.6, 0.25, 0.15])[0]
            idx = random.randint(0, len(new_tokens) - 1)

            if op == "substitute":
                current = new_tokens[idx]
                if current in BIOISOSTERE_REPLACEMENTS and random.random() < 0.6:
                    new_tokens[idx] = random.choice(BIOISOSTERE_REPLACEMENTS[current])
                else:
                    new_tokens[idx] = random.choice(PHARMACOPHORE_TOKENS)

            elif op == "insert":
                insert_token = random.choice(PHARMACOPHORE_TOKENS)
                new_tokens.insert(idx, insert_token)

            elif op == "delete" and len(new_tokens) > 6:
                del new_tokens[idx]

        mutated = "".join(new_tokens)
        return mutated
    except Exception:
        return None


def crossover_selfies(parent1_selfies: str, parent2_selfies: str) -> Optional[str]:
    """
    Effectue un crossover génétique (recombinaison) entre deux chaînes SELFIES.
    Garantit toujours un descendant chimiquement valide à 100%.
    """
    try:
        tokens1 = list(sf.split_selfies(parent1_selfies))
        tokens2 = list(sf.split_selfies(parent2_selfies))
        if len(tokens1) < 4 or len(tokens2) < 4:
            return None

        cut1 = random.randint(1, len(tokens1) - 2)
        cut2 = random.randint(1, len(tokens2) - 2)

        child_tokens = tokens1[:cut1] + tokens2[cut2:]
        return "".join(child_tokens)
    except Exception:
        return None


def generate_selfies_batch(seeds: List[str], count: int = 50, max_mutations: int = 2) -> List[str]:
    """
    Génère un lot de nouvelles molécules uniques par mutation et recombinaison SELFIES
    à partir d'une liste de molécules graines (SMILES).
    """
    selfies_seeds = []
    for s in seeds:
        encoded = smiles_to_selfies(s)
        if encoded:
            selfies_seeds.append(encoded)

    if not selfies_seeds:
        return []

    candidates = set()
    attempts = 0
    max_attempts = count * 15

    while len(candidates) < count and attempts < max_attempts:
        attempts += 1
        
        # 75% mutation, 25% crossover si au moins 2 graines
        if len(selfies_seeds) >= 2 and random.random() < 0.25:
            p1, p2 = random.sample(selfies_seeds, 2)
            child_selfies = crossover_selfies(p1, p2)
        else:
            p = random.choice(selfies_seeds)
            child_selfies = mutate_selfies(p, max_mutations=max_mutations)

        if not child_selfies:
            continue

        smi = selfies_to_smiles(child_selfies)
        if smi and smi not in seeds and smi not in candidates:
            # Vérifier que la molécule n'est pas trop petite ou trop grande
            mol = Chem.MolFromSmiles(smi)
            if mol and 10 <= mol.GetNumHeavyAtoms() <= 50:
                candidates.add(smi)

    return list(candidates)
