//! Evaluation session and report generation.
//!
//! This module provides the core evaluation infrastructure:
//!
//! - [`session::EvalSession`]: Main evaluation session with codec callbacks
//! - [`session::EvalConfig`]: Configuration for evaluation
//! - [`session::ImageData`]: Image data types accepted by the session
//! - [`report`]: Report types for evaluation results
//! - [`helpers`]: Lightweight helpers for simple quality evaluation

pub mod helpers;
pub mod report;
pub mod session;

pub use helpers::{assert_perception_level, assert_quality, evaluate_single};
pub use report::{CodecResult, CorpusReport, ImageReport};
pub use session::{EvalConfig, EvalSession, ImageData};
