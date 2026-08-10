#!/usr/bin/env bash
# chewie-env rootfs 빌드 (ARCHITECTURE.md §8.2).
#
#   ./rootfs/build.sh [버전]
#
# 산출물: dist-rootfs/chewie-rootfs-<버전>.tar.gz 와 .sha256
#
# 이 tar.gz 는 인스톨러에 그대로 동봉된다 (`src-tauri/tauri.bundle.json` → §8.1).
# 경로·파일명이 그 설정과 일치해야 하고, 다시 빌드하면 체크섬이 반드시 바뀌므로
# `src-tauri/src/settings.rs` 의 기본 SHA256 도 함께 갱신해야 한다.
#
# Docker 는 여기서만 쓴다. 사용자 PC 에는 필요 없다.

set -euo pipefail

VERSION="${1:-3.5.4}"
IMAGE="chewie-rootfs:${VERSION}"
CONTAINER="chewie-rootfs-export-$$"
OUT_DIR="$(cd "$(dirname "$0")/.." && pwd)/dist-rootfs"
TARBALL="${OUT_DIR}/chewie-rootfs-${VERSION}.tar.gz"

cd "$(dirname "$0")"
mkdir -p "$OUT_DIR"

echo "==> 이미지 빌드: ${IMAGE}"
docker build -t "$IMAGE" .

echo "==> rootfs 추출"
# `docker export` 는 이미지가 아니라 컨테이너의 파일시스템을 뽑는다.
# 레이어 메타데이터 없이 순수 파일 트리가 나와야 wsl --import 가 받아들인다.
docker create --name "$CONTAINER" "$IMAGE" >/dev/null
trap 'docker rm -f "$CONTAINER" >/dev/null 2>&1 || true' EXIT
docker export "$CONTAINER" | gzip -9 > "$TARBALL"

echo "==> 체크섬"
( cd "$OUT_DIR" && sha256sum "$(basename "$TARBALL")" > "$(basename "$TARBALL").sha256" )

echo
echo "완료:"
ls -lh "$TARBALL" "$TARBALL.sha256"
echo
echo "SHA256:"
cat "$TARBALL.sha256"
echo
echo "이 값을 src-tauri/src/settings.rs 의 기본 sha256 에 넣어야"
echo "npm run tauri:build 로 만든 인스톨러가 동봉본 검증을 통과합니다."
echo "(임시 확인은 앱 [설정] → rootfs 이미지 칸에 위 경로를 넣어도 됩니다)"
