"""Smoke tests for the Axiom Python bindings.

These mirror, in Python, a few of the Rust property tests: convergence under
merge, add-wins, value-can-decrease, sequence convergence, and a MessagePack
round-trip (the distributed-state path).
"""

from axiom import GCounter, PNCounter, ORSet, RGA


def test_gcounter_merge_converges():
    a = GCounter(1)
    b = GCounter(2)
    a.increment()
    a.increment()
    b.increment()
    # Merge in both directions; values must agree.
    ab = GCounter.from_bytes(a.to_bytes())
    ab.merge(b)
    ba = GCounter.from_bytes(b.to_bytes())
    ba.merge(a)
    assert ab.value() == ba.value() == 3


def test_pncounter_value_can_decrease():
    c = PNCounter(1)
    c.increment()
    assert c.value() == 1
    c.decrement()
    c.decrement()
    assert c.value() == -1


def test_orset_concurrent_add_beats_remove():
    a = ORSet()
    a.add("x")  # tag t1
    b = ORSet.from_bytes(a.to_bytes())  # b observes t1
    a.add("x")  # concurrent add, fresh tag
    b.discard("x")  # tombstones only the observed t1
    a.merge(b)
    assert "x" in a  # add wins


def test_orset_remove_wins_when_observed():
    a = ORSet()
    a.add("y")
    b = ORSet.from_bytes(a.to_bytes())
    b.discard("y")  # observed the only tag
    a.merge(b)
    assert "y" not in a


def test_rga_merge_is_order_independent():
    a = RGA(1)
    a.append("h")
    a.append("i")
    b = RGA(2)
    b.append("y")
    ab = RGA.from_bytes(a.to_bytes())
    ab.merge(b)
    ba = RGA.from_bytes(b.to_bytes())
    ba.merge(a)
    assert ab.to_list() == ba.to_list()


def test_rga_insert_order_and_delete():
    r = RGA(1)
    r.insert(0, "a")
    r.insert(1, "b")
    r.insert(1, "c")  # newest-after-reference wins the spot
    assert r.to_list() == ["a", "c", "b"]
    r.delete(1)  # delete "c"
    assert r.to_list() == ["a", "b"]


def test_messagepack_roundtrip_all_types():
    g = GCounter(1)
    g.increment()
    assert GCounter.from_bytes(g.to_bytes()).value() == g.value()

    s = ORSet()
    s.add("k")
    assert "k" in ORSet.from_bytes(s.to_bytes())

    r = RGA(1)
    r.append("z")
    assert RGA.from_bytes(r.to_bytes()).to_list() == ["z"]
