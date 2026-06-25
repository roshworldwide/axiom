# axiom (Python bindings)

Formally specified CRDTs for Python — a thin [PyO3](https://pyo3.rs) wrapper over
the Rust [`axiom-core`](../axiom-core). Each type is specified in TLA+
(model-checked with TLC; G-Counter merge and acoustic-auth freshness
machine-proved with TLAPS) and property-tested in Rust.

## Why

Multi-agent and multi-process systems — LangChain, AutoGen, and similar — often
need **shared state that several agents update concurrently without a
coordinator**: a running tally, a set of facts, a collaboratively-edited
document. CRDTs converge automatically. Each agent keeps a replica, exchanges
serialized bytes over whatever transport it has, and `merge`s; any two replicas
that have seen each other's updates (in any order) reach the same state.

## Build

```sh
python -m venv .venv && source .venv/bin/activate
pip install maturin pytest
maturin develop      # compiles the Rust extension, installs it into the venv
pytest               # smoke tests
```

## Quickstart

```python
from axiom import GCounter, PNCounter, ORSet, RGA

# A grow-only counter replicated across two agents.
a = GCounter(replica_id=1)
b = GCounter(replica_id=2)
a.increment(); a.increment()
b.increment()

# Agents exchange serialized state over any transport, then merge.
b.merge(GCounter.from_bytes(a.to_bytes()))
a.merge(GCounter.from_bytes(b.to_bytes()))
assert a.value() == b.value() == 3        # converged, no coordinator

# An add-wins set of facts shared between agents:
facts = ORSet()
facts.add("user_prefers_dark_mode")
assert "user_prefers_dark_mode" in facts

# A collaboratively-edited sequence (RGA):
doc = RGA(replica_id=1)
doc.append("hello"); doc.append("world")
assert doc.to_list() == ["hello", "world"]
```

## API

| Type | Constructor | Methods |
|------|-------------|---------|
| `GCounter` | `GCounter(replica_id)` | `increment()`, `value()`, `merge(other)` |
| `PNCounter` | `PNCounter(replica_id)` | `increment()`, `decrement()`, `value()`, `merge(other)` |
| `ORSet` | `ORSet()` | `add(s)`, `discard(s)`, `s in set`, `len(set)`, `elements()`, `merge(other)` |
| `RGA` | `RGA(replica_id)` | `insert(i, s)`, `append(s)`, `delete(i)`, `len(seq)`, `to_list()`, `merge(other)` |

Every type supports `to_bytes()` / `from_bytes(data)` (MessagePack) for
replication. Convergence is the property the underlying TLA+ specs verify and the
Rust suite property-tests.
