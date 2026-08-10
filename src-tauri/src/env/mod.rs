//! Environment Provisioner (§4.3, §7).
//!
//! 온보딩은 가장 많은 이탈이 발생하는 구간이다. 이 모듈의 설계 목표는 두 가지다.
//!
//! * **정상 환경 사용자에게는 아무것도 묻지 않는다** — 검사부터 하지 않고
//!   실제 실행을 먼저 시도한다.
//! * **돌이킬 수 없는 조작 전에 불가능한 기기를 먼저 거른다** — 기능 활성화와
//!   재부팅을 시키기 *전에* 하드웨어 게이트를 통과시킨다. Docker Desktop 이
//!   재부팅 후에야 `0x80370102` 로 실패하는 경로를 피하기 위한 것이다(§7.2).

pub mod probe;
pub mod provision;

pub use probe::{probe, EnvReport};
pub use provision::{DownloadProgress, Provisioner, RootfsOrigin, RootfsSource};
