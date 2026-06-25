//! Python bindings for the Axiom CRDT runtime (via PyO3 + maturin).
//!
//! Exposes the four CRDTs with Pythonic APIs and MessagePack `to_bytes` /
//! `from_bytes`, so a Python process can replicate shared state: serialize a
//! replica, ship the bytes to a peer, and `merge` — convergence is guaranteed by
//! the (TLA+-specified, property-tested) `axiom-core` underneath.

use axiom_core::{GCounter, ORSet, PNCounter, ReplicaId, Rga};
use pyo3::exceptions::{PyIndexError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;

fn to_bytes_err<E: std::fmt::Display>(e: E) -> PyErr {
    PyValueError::new_err(format!("serialization failed: {e}"))
}

/// A grow-only counter.
#[pyclass(name = "GCounter")]
struct PyGCounter {
    inner: GCounter,
}

#[pymethods]
impl PyGCounter {
    #[new]
    fn new(replica_id: u64) -> Self {
        Self {
            inner: GCounter::new(ReplicaId(replica_id)),
        }
    }

    /// Increment this replica's component by one.
    fn increment(&mut self) {
        self.inner.increment();
    }

    /// The counter's value (sum of components).
    fn value(&self) -> u64 {
        self.inner.value()
    }

    /// Merge another replica's state in (component-wise max).
    fn merge(&mut self, other: &PyGCounter) {
        self.inner.merge(&other.inner);
    }

    /// MessagePack-encode this replica's state.
    fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let bytes = rmp_serde::to_vec(&self.inner).map_err(to_bytes_err)?;
        Ok(PyBytes::new(py, &bytes))
    }

    /// Decode a replica from MessagePack bytes.
    #[staticmethod]
    fn from_bytes(data: &[u8]) -> PyResult<Self> {
        Ok(Self {
            inner: rmp_serde::from_slice(data).map_err(to_bytes_err)?,
        })
    }

    fn __repr__(&self) -> String {
        format!("GCounter(value={})", self.inner.value())
    }
}

/// A counter supporting increment and decrement.
#[pyclass(name = "PNCounter")]
struct PyPNCounter {
    inner: PNCounter,
}

#[pymethods]
impl PyPNCounter {
    #[new]
    fn new(replica_id: u64) -> Self {
        Self {
            inner: PNCounter::new(ReplicaId(replica_id)),
        }
    }

    fn increment(&mut self) {
        self.inner.increment();
    }

    fn decrement(&mut self) {
        self.inner.decrement();
    }

    /// The net value `P - N` (may be negative).
    fn value(&self) -> i64 {
        self.inner.value()
    }

    fn merge(&mut self, other: &PyPNCounter) {
        self.inner.merge(&other.inner);
    }

    fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let bytes = rmp_serde::to_vec(&self.inner).map_err(to_bytes_err)?;
        Ok(PyBytes::new(py, &bytes))
    }

    #[staticmethod]
    fn from_bytes(data: &[u8]) -> PyResult<Self> {
        Ok(Self {
            inner: rmp_serde::from_slice(data).map_err(to_bytes_err)?,
        })
    }

    fn __repr__(&self) -> String {
        format!("PNCounter(value={})", self.inner.value())
    }
}

/// An observed-remove set of strings (add-wins).
#[pyclass(name = "ORSet")]
struct PyORSet {
    inner: ORSet<String>,
}

#[pymethods]
impl PyORSet {
    #[new]
    fn new() -> Self {
        Self {
            inner: ORSet::new(),
        }
    }

    fn add(&mut self, element: String) {
        self.inner.add(element);
    }

    /// Remove an element (tombstone its observed tags).
    fn discard(&mut self, element: &str) {
        self.inner.remove(&element.to_owned());
    }

    fn contains(&self, element: &str) -> bool {
        self.inner.contains(&element.to_owned())
    }

    fn __contains__(&self, element: &str) -> bool {
        self.contains(element)
    }

    /// The present elements (in sorted order).
    fn elements(&self) -> Vec<String> {
        self.inner.iter().cloned().collect()
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn merge(&mut self, other: &PyORSet) {
        self.inner.merge(&other.inner);
    }

    fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let bytes = rmp_serde::to_vec(&self.inner).map_err(to_bytes_err)?;
        Ok(PyBytes::new(py, &bytes))
    }

    #[staticmethod]
    fn from_bytes(data: &[u8]) -> PyResult<Self> {
        Ok(Self {
            inner: rmp_serde::from_slice(data).map_err(to_bytes_err)?,
        })
    }

    fn __repr__(&self) -> String {
        format!("ORSet({:?})", self.elements())
    }
}

/// A replicated growable array (sequence) of strings.
#[pyclass(name = "RGA")]
struct PyRga {
    inner: Rga<String>,
}

#[pymethods]
impl PyRga {
    #[new]
    fn new(replica_id: u64) -> Self {
        Self {
            inner: Rga::new(ReplicaId(replica_id)),
        }
    }

    /// Insert `value` at visible position `index` (clamped to the end).
    fn insert(&mut self, index: usize, value: String) {
        self.inner.insert(index, value);
    }

    /// Append `value` to the end.
    fn append(&mut self, value: String) {
        let n = self.inner.len();
        self.inner.insert(n, value);
    }

    /// Tombstone the element at visible position `index`.
    fn delete(&mut self, index: usize) -> PyResult<()> {
        let ids = self.inner.ids();
        let id = ids
            .get(index)
            .ok_or_else(|| PyIndexError::new_err("RGA index out of range"))?;
        self.inner.delete(*id);
        Ok(())
    }

    /// The visible sequence as a list of strings.
    fn to_list(&self) -> Vec<String> {
        self.inner.to_vec().into_iter().cloned().collect()
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn merge(&mut self, other: &PyRga) {
        self.inner.merge(&other.inner);
    }

    fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let bytes = rmp_serde::to_vec(&self.inner).map_err(to_bytes_err)?;
        Ok(PyBytes::new(py, &bytes))
    }

    #[staticmethod]
    fn from_bytes(data: &[u8]) -> PyResult<Self> {
        Ok(Self {
            inner: rmp_serde::from_slice(data).map_err(to_bytes_err)?,
        })
    }

    fn __repr__(&self) -> String {
        format!("RGA({:?})", self.to_list())
    }
}

/// The `axiom` Python module.
#[pymodule]
fn axiom(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyGCounter>()?;
    m.add_class::<PyPNCounter>()?;
    m.add_class::<PyORSet>()?;
    m.add_class::<PyRga>()?;
    Ok(())
}
