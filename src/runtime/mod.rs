pub mod containerd;
pub mod detector;
pub mod docker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeType {
    Containerd,
    Docker,
}
