//! Prodigal training file(`.trn`) 저장소.
//!
//! `%LOCALAPPDATA%\ChewieApp\training\` 에 모은다. 스키마(WSL 내부)와 반대로
//! **Windows 쪽**에 두는 이유는 두 가지다 — 파일 하나뿐이라 9p 비용이 없고,
//! 사용자가 백업하거나 다른 PC 로 옮길 수 있어야 한다. 앱 소유 영역이므로
//! 언인스톨 훅이 이미 정리한다(`nsis/hooks.nsh` 를 손댈 필요가 없다).
//!
//! **덮어쓰기를 하지 않는다.** 같은 이름이 있으면 거절한다. 확인을 받을 UI 가
//! 없는 채널(MCP)에서도 부르기 때문이고, 그 덕에 이 저장소에는 되돌릴 수 없는
//! 조작이 `delete` 하나뿐이다(그쪽은 UI 전용이다).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::error::{Error, Result};
use crate::fasta::{self, GenomeScan, GenomeStat};
use crate::runner::ChewieRunner;

/// 저장소에 있는 training file 하나.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainingFile {
    /// 확장자를 뺀 이름. 저장소 안에서 유일하며 삭제할 때의 키다.
    pub name: String,
    /// `--ptf` 에 그대로 넣는 Windows 절대 경로.
    pub path: String,
    pub created_at: String,
    pub size_bytes: u64,
}

/// 새로 만든 결과. **무엇으로 학습했는지를 함께 돌려준다** — 앱이 고른 것이라
/// 사용자(와 모델)가 그 선택을 확인할 수 있어야 한다.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainingCreated {
    pub file: TrainingFile,
    pub picked: GenomeStat,
    pub reason: String,
}

/// 파일 이름 길이 상한. Windows 의 260자 경로 제한에 여유를 둔 값이다.
const MAX_NAME: usize = 64;

pub struct TrainingStore {
    dir: PathBuf,
    runner: Arc<dyn ChewieRunner>,
}

impl TrainingStore {
    pub fn new(dir: PathBuf, runner: Arc<dyn ChewieRunner>) -> Self {
        Self { dir, runner }
    }

    /// 저장소의 `.trn` 목록. 이름 순으로 정렬한다.
    ///
    /// DB 를 쓰지 않는다 — 디렉터리가 진실이고, 사용자가 파일을 직접 넣거나 빼도
    /// 목록이 따라오는 편이 낫다(외부에서 받은 `.trn` 을 그냥 복사해 넣는 것이
    /// 실제로 가장 흔한 사용법이다).
    pub fn list(&self) -> Result<Vec<TrainingFile>> {
        let mut out = Vec::new();
        // 폴더가 아직 없을 수 있다 (구버전에서 올라온 설치). 빈 목록으로 본다.
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Ok(out);
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path
                .extension()
                .map(|e| !e.eq_ignore_ascii_case("trn"))
                .unwrap_or(true)
            {
                continue;
            }
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let meta = entry.metadata().ok();
            out.push(TrainingFile {
                name,
                path: path.to_string_lossy().to_string(),
                created_at: meta
                    .as_ref()
                    .and_then(|m| m.modified().ok())
                    .map(iso_from)
                    .unwrap_or_default(),
                size_bytes: meta.map(|m| m.len()).unwrap_or(0),
            });
        }

        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(out)
    }

    /// 폴더를 훑어 학습에 쓸 후보를 추린다. 파일을 만들지 않는 읽기 전용 단계다.
    pub fn scan(&self, genome_dir: &Path) -> Result<GenomeScan> {
        fasta::scan_dir(genome_dir)
    }

    /// 게놈 폴더에서 하나를 골라 학습시키고 저장소에 넣는다.
    ///
    /// `genome_file` 을 주면 그것을 쓰고, 없으면 `scan` 의 1위를 쓴다. UI 는
    /// 사용자가 후보를 보고 고른 뒤 명시적으로 넘기고, MCP 는 생략해 앱이
    /// 고르게 한다 — 어느 쪽이든 같은 경로를 지난다.
    pub fn create(
        &self,
        name: &str,
        genome_dir: &Path,
        genome_file: Option<&Path>,
    ) -> Result<TrainingCreated> {
        let stem = safe_stem(name)?;
        let dest = self.dir.join(format!("{stem}.trn"));

        // **실행 전에 막는다.** pyrodigal 의 `-t` 는 파일이 이미 있으면 쓰지 않고
        // **읽는다.** 다른 게놈으로 다시 돌려도 exit 0 이고 파일은 그대로다
        // (2026-08-14 실측: md5 불변). 여기서 막지 않으면 사용자는 새로 만들어졌다고
        // 믿은 채 예전 모델을 계속 쓰게 된다 — 오류가 없어서 알아챌 방법이 없다.
        if dest.exists() {
            return Err(Error::InvalidInput(format!(
                "같은 이름의 training file 이 이미 있습니다: {stem}\n다른 이름을 쓰거나 [스키마] 화면에서 기존 것을 지운 뒤 다시 시도하세요."
            )));
        }

        let scan = fasta::scan_dir(genome_dir)?;
        let picked = match genome_file {
            Some(p) => scan
                .candidates
                .iter()
                .find(|c| Path::new(&c.path) == p)
                .cloned()
                // 후보 밖의 파일을 지정했을 수도 있다 — 크기 창에서 걸러졌지만
                // 사용자가 굳이 그것을 쓰겠다는 경우다. 존중하되 **다시 재서**
                // 넣는다. 0 으로 채우면 안내 문구가 "contig 0개, 0 kb" 가 된다.
                .or_else(|| p.is_file().then(|| fasta::measure(p).ok()).flatten())
                .ok_or_else(|| {
                    Error::InvalidInput(format!("게놈 파일을 찾을 수 없습니다: {}", p.display()))
                })?,
            None => scan.candidates[0].clone(),
        };

        std::fs::create_dir_all(&self.dir)?;
        self.runner
            .create_training_file(Path::new(&picked.path), &dest)?;

        // 산출물이 실제로 생겼는지 확인한다. pyrodigal 이 0 으로 끝나고도 파일을
        // 안 쓰는 경로가 있어(위의 `-t` 동작) 종료 코드만 믿지 않는다.
        let meta = std::fs::metadata(&dest).map_err(|_| {
            Error::Other(
                "학습은 끝났지만 training file 이 만들어지지 않았습니다.\n고른 게놈이 너무 짧거나 서열이 비어 있을 수 있습니다.".into(),
            )
        })?;

        Ok(TrainingCreated {
            file: TrainingFile {
                name: stem,
                path: dest.to_string_lossy().to_string(),
                created_at: meta.modified().ok().map(iso_from).unwrap_or_default(),
                size_bytes: meta.len(),
            },
            reason: if genome_file.is_some() {
                format!(
                    "지정한 게놈으로 학습했습니다 — {} (contig {}개, {}).",
                    picked.file_name,
                    picked.contigs,
                    fasta::human_bases(picked.bases)
                )
            } else {
                scan.reason.clone()
            },
            picked,
        })
    }

    /// 되돌릴 수 없다. UI 에서 확인을 받은 뒤 호출한다 (MCP 에는 노출하지 않는다).
    pub fn delete(&self, name: &str) -> Result<()> {
        let stem = safe_stem(name)?;
        let path = self.dir.join(format!("{stem}.trn"));
        if !path.is_file() {
            return Err(Error::InvalidInput(format!(
                "training file 을 찾을 수 없습니다: {stem}"
            )));
        }
        std::fs::remove_file(path)?;
        Ok(())
    }
}

fn iso_from(t: std::time::SystemTime) -> String {
    OffsetDateTime::from(t)
        .format(&Rfc3339)
        .unwrap_or_else(|_| String::new())
}

/// 사용자가 준 이름을 파일 이름으로 쓸 수 있는지 본다.
///
/// **`util::slugify` 를 쓰지 않는다.** 그쪽은 비ASCII 를 `x` 로 바꾸는데,
/// 스키마 **디렉터리 ID** 에는 맞아도 사용자가 직접 읽는 파일 이름에는 맞지
/// 않는다 — "비프라길리스" 가 "xxxxxxx.trn" 이 되면 목록에서 구별이 안 된다.
/// 그래서 Windows 가 금지하는 문자와 경로 탈출만 걸러내고 나머지는 살린다.
fn safe_stem(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(Error::InvalidInput(
            "training file 이름을 입력하세요 (예: B_fragilis)".into(),
        ));
    }
    if trimmed.chars().count() > MAX_NAME {
        return Err(Error::InvalidInput(format!(
            "이름이 너무 깁니다 ({MAX_NAME}자 이내로 입력하세요)"
        )));
    }
    if trimmed.contains("..") {
        return Err(Error::InvalidInput(
            "이름에 `..` 을 쓸 수 없습니다".into(),
        ));
    }
    if let Some(bad) = trimmed
        .chars()
        .find(|c| r#"<>:"/\|?*"#.contains(*c) || c.is_control())
    {
        return Err(Error::InvalidInput(format!(
            "이름에 쓸 수 없는 문자가 있습니다: {bad}\n\\ / : * ? \" < > | 는 파일 이름에 넣을 수 없습니다."
        )));
    }
    // 확장자를 붙여주므로 사용자가 함께 적었으면 걷어낸다.
    let stem = trimmed.strip_suffix(".trn").unwrap_or(trimmed).trim();
    if stem.is_empty() {
        return Err(Error::InvalidInput(
            "확장자를 뺀 이름이 필요합니다 (예: B_fragilis)".into(),
        ));
    }
    Ok(stem.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_korean_names_intact() {
        // slugify 였다면 "xxxxx" 가 된다. 목록에서 구별되어야 하므로 살린다.
        assert_eq!(safe_stem("비프라길리스").unwrap(), "비프라길리스");
        assert_eq!(safe_stem("  B_fragilis  ").unwrap(), "B_fragilis");
    }

    #[test]
    fn strips_a_redundant_extension() {
        // 사용자가 "B_fragilis.trn" 을 적으면 "B_fragilis.trn.trn" 이 되면 안 된다.
        assert_eq!(safe_stem("B_fragilis.trn").unwrap(), "B_fragilis");
    }

    #[test]
    fn rejects_path_escapes_and_illegal_characters() {
        for bad in ["../../etc/passwd", "a/b", "a\\b", "a:b", "a?b", "  "] {
            assert!(safe_stem(bad).is_err(), "통과하면 안 된다: {bad}");
        }
    }
}
