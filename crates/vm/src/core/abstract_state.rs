//! Persistent versions for abstract EVM memory and storage.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use alloy::primitives::U256;

use super::symbolic::AbstractValue;

/// Maximum predecessor roots retained before a state join widens to unknown.
pub const MAX_STATE_JOIN_PARENTS: usize = 8;

/// Distinguishes independently versioned EVM state spaces.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StateDomain {
    /// Transient byte-addressed memory.
    Memory,
    /// Persistent contract storage.
    Storage,
    /// Transaction-scoped transient storage.
    TransientStorage,
}

/// Stable identifier for a persistent abstract-state version.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StateVersionId(usize);

impl StateVersionId {
    /// Return this version's arena index.
    pub fn index(self) -> usize {
        self.0
    }
}

/// One node in a persistent state-version DAG.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum StateVersionNode {
    /// Unknown initial contents of one EVM state space.
    Initial(StateDomain),
    /// A widened state whose predecessor versions are no longer distinguished.
    Unknown(StateDomain),
    /// A write derived from a prior version.
    Write {
        /// State space being modified.
        domain: StateDomain,
        /// Version before the write.
        parent: StateVersionId,
        /// Abstract byte offset or storage key.
        key: AbstractValue,
        /// Value written, when represented by the instruction.
        value: Option<AbstractValue>,
        /// Number of bytes written when statically known.
        size: Option<usize>,
    },
    /// Join of versions reaching the same CFG point.
    Join {
        /// State space being merged.
        domain: StateDomain,
        /// Alternative predecessor versions.
        parents: BTreeSet<StateVersionId>,
    },
}

/// Hash-consed persistent state-version DAG.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateVersionArena {
    nodes: Vec<StateVersionNode>,
    interned: HashMap<StateVersionNode, StateVersionId>,
}

impl Default for StateVersionArena {
    fn default() -> Self {
        let mut arena = Self { nodes: Vec::new(), interned: HashMap::new() };
        for domain in [StateDomain::Memory, StateDomain::Storage, StateDomain::TransientStorage] {
            arena.intern(StateVersionNode::Initial(domain));
        }
        arena
    }
}

impl StateVersionArena {
    /// Construct an arena containing the three canonical initial roots.
    pub fn new() -> Self {
        Self::default()
    }

    /// Canonical initial version for a state space.
    pub fn initial(&self, domain: StateDomain) -> StateVersionId {
        match domain {
            StateDomain::Memory => StateVersionId(0),
            StateDomain::Storage => StateVersionId(1),
            StateDomain::TransientStorage => StateVersionId(2),
        }
    }

    /// Intern a state-version node.
    pub fn intern(&mut self, node: StateVersionNode) -> StateVersionId {
        if let Some(version) = self.interned.get(&node) {
            return *version
        }
        let version = StateVersionId(self.nodes.len());
        self.nodes.push(node.clone());
        self.interned.insert(node, version);
        version
    }

    /// Look up a state-version node.
    pub fn get(&self, version: StateVersionId) -> Option<&StateVersionNode> {
        self.nodes.get(version.0)
    }

    /// Number of unique state versions.
    pub fn version_count(&self) -> usize {
        self.nodes.len()
    }

    fn join(
        &mut self,
        domain: StateDomain,
        left: StateVersionId,
        right: StateVersionId,
    ) -> StateVersionId {
        if left == right {
            return left
        }
        if matches!(self.get(left), Some(StateVersionNode::Unknown(node_domain)) if *node_domain == domain)
        {
            return left
        }
        if matches!(self.get(right), Some(StateVersionNode::Unknown(node_domain)) if *node_domain == domain)
        {
            return right
        }
        let mut parents = BTreeSet::new();
        self.collect_join_parents(domain, left, &mut parents);
        self.collect_join_parents(domain, right, &mut parents);
        if parents.len() > MAX_STATE_JOIN_PARENTS {
            self.intern(StateVersionNode::Unknown(domain))
        } else {
            self.intern(StateVersionNode::Join { domain, parents })
        }
    }

    fn collect_join_parents(
        &self,
        domain: StateDomain,
        version: StateVersionId,
        parents: &mut BTreeSet<StateVersionId>,
    ) {
        match self.get(version) {
            Some(StateVersionNode::Join { domain: node_domain, parents: nested })
                if *node_domain == domain =>
            {
                parents.extend(nested);
            }
            _ => {
                parents.insert(version);
            }
        }
    }
}

/// One versioned abstract memory or storage space with exact-key forwarding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionedState {
    /// State space represented by this value.
    pub domain: StateDomain,
    /// Persistent version root.
    pub version: StateVersionId,
    known: BTreeMap<AbstractValue, AbstractValue>,
}

impl VersionedState {
    /// Construct unknown initial state for a domain.
    pub fn initial(domain: StateDomain) -> Self {
        let version = match domain {
            StateDomain::Memory => StateVersionId(0),
            StateDomain::Storage => StateVersionId(1),
            StateDomain::TransientStorage => StateVersionId(2),
        };
        Self { domain, version, known: BTreeMap::new() }
    }

    /// Return a value forwarded from an exact matching write.
    pub fn load(&self, key: &AbstractValue) -> Option<&AbstractValue> {
        is_precise_key(key).then(|| self.known.get(key)).flatten()
    }

    /// Record a write and advance the persistent version.
    pub fn store(
        &mut self,
        key: AbstractValue,
        value: Option<AbstractValue>,
        size: Option<usize>,
        versions: &mut StateVersionArena,
    ) {
        self.version = versions.intern(StateVersionNode::Write {
            domain: self.domain,
            parent: self.version,
            key: key.clone(),
            value: value.clone(),
            size,
        });
        if !is_precise_key(&key) {
            self.known.clear();
            return
        }

        match (self.domain, constant_key(&key), size) {
            (StateDomain::Memory, Some(offset), Some(size)) => {
                self.known.retain(|known_key, _| {
                    constant_key(known_key)
                        .is_some_and(|known_offset| !ranges_overlap(offset, size, known_offset, 32))
                });
            }
            (StateDomain::Memory, ..) => self.known.clear(),
            (_, Some(concrete), _) => {
                self.known.retain(|known_key, _| constant_key(known_key) != Some(concrete));
            }
            (_, None, _) => self.known.clear(),
        }

        if self.domain != StateDomain::Memory || size == Some(32) {
            if let Some(value) = value {
                self.known.insert(key, value);
            }
        }
    }

    /// Forget values overlapping an unknown write while retaining a distinct version.
    pub fn havoc(
        &mut self,
        key: AbstractValue,
        size: Option<usize>,
        versions: &mut StateVersionArena,
    ) {
        self.store(key, None, size, versions);
    }

    pub(crate) fn join(
        &self,
        other: &Self,
        max_values: usize,
        versions: &mut StateVersionArena,
    ) -> Self {
        debug_assert_eq!(self.domain, other.domain);
        let known = self
            .known
            .iter()
            .filter_map(|(key, left)| {
                let right = other.known.get(key)?;
                Some((key.clone(), left.join(right, max_values)))
            })
            .collect();
        Self {
            domain: self.domain,
            version: versions.join(self.domain, self.version, other.version),
            known,
        }
    }
}

/// Versioned memory, persistent storage, and transient storage in an abstract state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbstractStateSpaces {
    /// EVM memory.
    pub memory: VersionedState,
    /// Contract storage.
    pub storage: VersionedState,
    /// EIP-1153 transient storage.
    pub transient_storage: VersionedState,
}

impl Default for AbstractStateSpaces {
    fn default() -> Self {
        Self {
            memory: VersionedState::initial(StateDomain::Memory),
            storage: VersionedState::initial(StateDomain::Storage),
            transient_storage: VersionedState::initial(StateDomain::TransientStorage),
        }
    }
}

impl AbstractStateSpaces {
    pub(crate) fn join(
        &self,
        other: &Self,
        max_values: usize,
        versions: &mut StateVersionArena,
    ) -> Self {
        Self {
            memory: self.memory.join(&other.memory, max_values, versions),
            storage: self.storage.join(&other.storage, max_values, versions),
            transient_storage: self.transient_storage.join(
                &other.transient_storage,
                max_values,
                versions,
            ),
        }
    }
}

fn constant_key(value: &AbstractValue) -> Option<U256> {
    value
        .known_values()
        .filter(|values| values.len() == 1)
        .and_then(|values| values.first().copied())
}

fn ranges_overlap(left: U256, left_size: usize, right: U256, right_size: usize) -> bool {
    let left_end = left.saturating_add(U256::from(left_size));
    let right_end = right.saturating_add(U256::from(right_size));
    left < right_end && right < left_end
}

fn is_precise_key(value: &AbstractValue) -> bool {
    match value {
        AbstractValue::Known(values) => values.len() == 1,
        AbstractValue::Symbolic { constants, expressions } => {
            constants.len() + expressions.len() == 1
        }
        AbstractValue::Unknown => false,
    }
}

#[cfg(test)]
mod tests {
    use alloy::primitives::U256;

    use super::*;
    use crate::core::{
        opcodes,
        symbolic::{ExpressionArena, ExpressionNode},
    };

    #[test]
    fn forwards_exact_storage_writes() {
        let mut versions = StateVersionArena::new();
        let mut storage = VersionedState::initial(StateDomain::Storage);
        let key = AbstractValue::constant(U256::from(3));
        let value = AbstractValue::constant(U256::from(7));
        storage.store(key.clone(), Some(value.clone()), Some(32), &mut versions);

        assert_eq!(storage.load(&key), Some(&value));
        assert_ne!(storage.version, versions.initial(StateDomain::Storage));
    }

    #[test]
    fn symbolic_alias_invalidates_exact_forwarding() {
        let mut versions = StateVersionArena::new();
        let mut storage = VersionedState::initial(StateDomain::Storage);
        let key = AbstractValue::constant(U256::from(3));
        storage.store(
            key.clone(),
            Some(AbstractValue::constant(U256::from(7))),
            Some(32),
            &mut versions,
        );
        storage.havoc(AbstractValue::Unknown, Some(32), &mut versions);

        assert_eq!(storage.load(&key), None);
    }

    #[test]
    fn overlapping_memory_write_invalidates_word_forwarding() {
        let mut versions = StateVersionArena::new();
        let mut memory = VersionedState::initial(StateDomain::Memory);
        let word = AbstractValue::constant(U256::ZERO);
        memory.store(
            word.clone(),
            Some(AbstractValue::constant(U256::from(7))),
            Some(32),
            &mut versions,
        );
        memory.store(
            AbstractValue::constant(U256::from(31)),
            Some(AbstractValue::constant(U256::from(1))),
            Some(1),
            &mut versions,
        );

        assert_eq!(memory.load(&word), None);
    }

    #[test]
    fn precise_memory_havoc_retains_non_overlapping_words() {
        let mut versions = StateVersionArena::new();
        let mut memory = VersionedState::initial(StateDomain::Memory);
        let first = AbstractValue::constant(U256::ZERO);
        let second = AbstractValue::constant(U256::from(64));
        memory.store(
            first.clone(),
            Some(AbstractValue::constant(U256::from(1))),
            Some(32),
            &mut versions,
        );
        memory.store(
            second.clone(),
            Some(AbstractValue::constant(U256::from(2))),
            Some(32),
            &mut versions,
        );
        memory.havoc(AbstractValue::constant(U256::from(16)), Some(32), &mut versions);

        assert_eq!(memory.load(&first), None);
        assert_eq!(memory.load(&second), Some(&AbstractValue::constant(U256::from(2))));
    }

    #[test]
    fn precise_symbolic_store_forwards_only_its_own_key() {
        let mut versions = StateVersionArena::new();
        let mut storage = VersionedState::initial(StateDomain::Storage);
        let mut expressions = ExpressionArena::new();
        let key = AbstractValue::expression(expressions.intern(ExpressionNode {
            opcode: opcodes::CALLER,
            inputs: vec![],
            output: 0,
            state_version: None,
            effect_site: None,
        }));
        let value = AbstractValue::constant(U256::from(7));
        storage.store(
            AbstractValue::constant(U256::from(3)),
            Some(AbstractValue::constant(U256::from(9))),
            Some(32),
            &mut versions,
        );
        storage.store(key.clone(), Some(value.clone()), Some(32), &mut versions);

        assert_eq!(storage.load(&key), Some(&value));
        assert_eq!(storage.load(&AbstractValue::constant(U256::from(3))), None);
    }

    #[test]
    fn joins_versions_and_only_shared_exact_values() {
        let mut versions = StateVersionArena::new();
        let mut left = VersionedState::initial(StateDomain::Storage);
        let mut right = VersionedState::initial(StateDomain::Storage);
        let shared = AbstractValue::constant(U256::from(1));
        left.store(
            shared.clone(),
            Some(AbstractValue::constant(U256::from(2))),
            Some(32),
            &mut versions,
        );
        right.store(
            shared.clone(),
            Some(AbstractValue::constant(U256::from(3))),
            Some(32),
            &mut versions,
        );
        left.store(
            AbstractValue::constant(U256::from(9)),
            Some(AbstractValue::constant(U256::from(4))),
            Some(32),
            &mut versions,
        );
        let joined = left.join(&right, 8, &mut versions);

        assert_eq!(
            joined.load(&shared).and_then(AbstractValue::known_values),
            Some(&BTreeSet::from([U256::from(2), U256::from(3)]))
        );
        assert!(matches!(versions.get(joined.version), Some(StateVersionNode::Join { .. })));
        assert_eq!(joined.load(&AbstractValue::constant(U256::from(9))), None);
    }
}
