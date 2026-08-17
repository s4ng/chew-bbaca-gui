import type { Messages } from "./messages/ko";
import type { Module } from "./types";

/**
 * 표시용 서식.
 *
 * 라벨 문자열은 여기 없다 — `lib/messages/` 로 옮겼다. 여기 남은 것은 **언어와
 * 무관한 계산**과, 카탈로그를 받아 조립만 하는 함수들이다.
 */

/**
 * 표준 파이프라인에서의 위치. 순서가 실제로 의미를 갖는 곳이라 번호를 붙인다.
 *
 * `PrepExternalSchema` 는 4단계가 아니라 **1단계의 대안**이다 — 스키마를 직접
 * 만드는 대신 남이 만든 것을 들여온다. 그래서 같은 번호를 준다.
 */
export const MODULE_STEP: Record<Module, number> = {
  CreateSchema: 1,
  PrepExternalSchema: 1,
  AlleleCall: 2,
  ExtractCgMLST: 3,
  // 아래는 표준 3단계 바깥의 후처리·점검 도구다. 4로 묶어 "그다음" 을 뜻한다.
  RemoveGenes: 4,
  JoinProfiles: 4,
  SchemaEvaluator: 4,
  AlleleCallEvaluator: 4,
};

/** DB 에는 UTC 로 저장하고 표시 직전에만 로컬로 바꾼다. */
export function formatTime(iso: string | null, t: Messages): string {
  if (!iso) return t.common.dash;
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(t.dateLocale, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function formatDuration(from: string | null, to: string | null, t: Messages): string {
  if (!from) return t.common.dash;
  const start = new Date(from).getTime();
  const end = to ? new Date(to).getTime() : Date.now();
  if (Number.isNaN(start) || Number.isNaN(end)) return t.common.dash;
  const sec = Math.max(0, Math.round((end - start) / 1000));
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = sec % 60;
  if (h > 0) return t.duration.hm(h, m);
  if (m > 0) return t.duration.ms(m, s);
  return t.duration.s(s);
}

/**
 * 단위가 B/KB/MB 라 언어와 무관하다. 카탈로그를 받지 않는 유일한 서식 함수이므로
 * 값이 없을 때의 `—` 도 여기서 그대로 쓴다.
 */
export function formatBytes(bytes: number | null): string {
  if (bytes == null) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i += 1;
  }
  return `${value.toFixed(value >= 100 || i === 0 ? 0 : 1)} ${units[i]}`;
}

/** 세균 게놈은 Mb 단위다. Rust `fasta::human_bases` 와 같은 규칙. */
export function formatBases(bases: number): string {
  return bases >= 1_000_000
    ? `${(bases / 1_000_000).toFixed(2)} Mb`
    : `${Math.round(bases / 1000)} kb`;
}
