import type { JobStatus, Module } from "./types";

export const MODULE_LABEL: Record<Module, string> = {
  CreateSchema: "스키마 생성",
  AlleleCall: "Allele calling",
  ExtractCgMLST: "core genome 추출",
  PrepExternalSchema: "외부 스키마 들여오기",
};

/**
 * [새 작업] 화면에서 모듈을 고를 때 보여줄 설명.
 *
 * 대상은 chewBBACA 를 처음 쓰는 사람이다. 무엇을 넣고 무엇이 나오는지, 그리고
 * **그다음에 뭘 해야 하는지**까지 적는다 — 이 앱에서 가장 흔한 막힘이
 * "돌리긴 했는데 이게 끝인가?" 이기 때문이다.
 */
export interface ModuleInfo {
  /** 한두 문장. 전문용어를 쓰면 곧바로 풀어 쓴다. */
  summary: string;
  needs: string;
  gives: string;
  /** 이 작업이 끝난 뒤 이어지는 단계 */
  next?: string;
  /** 모르고 지나가면 손해 보는 것 */
  caution?: string;
}

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
};

/** 단계 표시에 쓸 이름. 1단계는 두 모듈이 공유하므로 중립적으로 적는다. */
export const STEP_LABEL: Record<number, string> = {
  1: "1. 스키마 준비",
  2: "2. Allele calling",
  3: "3. core genome 추출",
};

export const MODULE_INFO: Record<Module, ModuleInfo> = {
  CreateSchema: {
    summary:
      "여러 균주의 게놈을 훑어 비교에 쓸 유전자 자리(loci) 목록을 만듭니다. 앞으로의 모든 분석이 이 목록을 기준으로 이뤄지므로, 균주를 비교하기 위한 '설문지'를 만드는 단계라고 보면 됩니다.",
    needs: "어셈블리 FASTA 파일들이 담긴 폴더",
    gives: "스키마 — loci 하나당 FASTA 파일 하나. 앱 저장소(WSL 내부)에 보관됩니다",
    next: "만든 스키마로 AlleleCall 을 실행해 균주별 프로파일을 얻습니다.",
    caution:
      "같은 균종의 공인 스키마가 이미 있다면 새로 만들기보다 그것을 쓰는 편이 결과를 남과 비교하기에 좋습니다. (외부 스키마 들여오기는 아직 앱에 없습니다)",
  },
  AlleleCall: {
    summary:
      "각 균주가 스키마의 loci 마다 어떤 변종(allele)을 갖는지 판정해 번호를 매깁니다. 결과는 균주 한 줄, loci 한 칸짜리 표이고, 이 표의 숫자가 얼마나 겹치는지로 균주 사이의 거리를 잽니다.",
    needs: "어셈블리 폴더 + 사용할 스키마",
    gives: "results_alleles.tsv — 균주 × loci 프로파일 표. 결과 폴더로 회수됩니다",
    next: "ExtractCgMLST 로 core genome 을 추려야 균주 비교에 쓸 수 있는 표가 됩니다.",
    caution:
      "처음 보는 서열은 새 allele 로 등록되어 스키마에 계속 추가됩니다. 스키마가 실행할 때마다 자라는 것은 정상입니다.",
  },
  PrepExternalSchema: {
    summary:
      "이미 만들어져 있는 스키마를 chewBBACA 가 쓸 수 있는 형태로 바꿔 들여옵니다. 같은 균종에 공인된 스키마가 있다면 직접 만드는 것보다 이쪽이 낫습니다 — 남의 결과와 숫자를 맞춰볼 수 있기 때문입니다.",
    needs: "loci 마다 FASTA 파일 하나로 된 스키마 폴더",
    gives: "변환된 스키마. CreateSchema 로 만든 것과 똑같이 쓰입니다",
    next: "들여온 스키마로 AlleleCall 을 실행합니다.",
    caution:
      "이 앱이 [내보내기] 로 만든 폴더를 되돌리는 것이라면 이 모듈이 아니라 [스키마] 화면의 [불러오기] 를 쓰세요. 그쪽은 변환 없이 그대로 복원합니다.",
  },
  ExtractCgMLST: {
    summary:
      "AlleleCall 결과에서 '거의 모든 균주가 공통으로 가진' loci 만 골라냅니다. 전체 loci 표에는 한 균주에만 있는 유전자 때문에 빈칸이 많아 그대로는 균주끼리 공정하게 비교할 수 없습니다.",
    needs: "AlleleCall 이 만든 results_alleles.tsv 파일 하나",
    gives:
      "임계값별 cgMLST 프로파일 표와 loci 목록(cgMLSTschema95.txt 등), 그리고 요약 HTML",
    next: "나온 loci 목록을 AlleleCall 의 [일부 loci 만 대상으로] 칸에 넣어 다시 실행하면 cgMLST 프로파일이 완성됩니다.",
  },
};

export const STATUS_LABEL: Record<JobStatus, string> = {
  queued: "대기 중",
  running: "실행 중",
  completed: "완료",
  failed: "실패",
  cancelled: "취소됨",
};

/** DB 에는 UTC 로 저장하고 표시 직전에만 로컬로 바꾼다. */
export function formatTime(iso: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString("ko-KR", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function formatDuration(from: string | null, to: string | null): string {
  if (!from) return "—";
  const start = new Date(from).getTime();
  const end = to ? new Date(to).getTime() : Date.now();
  if (Number.isNaN(start) || Number.isNaN(end)) return "—";
  const sec = Math.max(0, Math.round((end - start) / 1000));
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = sec % 60;
  if (h > 0) return `${h}시간 ${m}분`;
  if (m > 0) return `${m}분 ${s}초`;
  return `${s}초`;
}

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
