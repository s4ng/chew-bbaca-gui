import type { JobStatus, Module } from "./types";

export const MODULE_LABEL: Record<Module, string> = {
  CreateSchema: "스키마 생성",
  AlleleCall: "Allele calling",
  ExtractCgMLST: "core genome 추출",
  PrepExternalSchema: "외부 스키마 들여오기",
  RemoveGenes: "loci 걸러내기",
  JoinProfiles: "결과 합치기",
  SchemaEvaluator: "스키마 리포트",
  AlleleCallEvaluator: "결과 리포트",
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
  // 아래는 표준 3단계 바깥의 후처리·점검 도구다. 4로 묶어 "그다음" 을 뜻한다.
  RemoveGenes: 4,
  JoinProfiles: 4,
  SchemaEvaluator: 4,
  AlleleCallEvaluator: 4,
};

/** 단계 표시에 쓸 이름. 1단계는 두 모듈이 공유하므로 중립적으로 적는다. */
export const STEP_LABEL: Record<number, string> = {
  1: "1. 스키마 준비",
  2: "2. Allele calling",
  3: "3. core genome 추출",
  4: "후처리 · 점검",
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
  RemoveGenes: {
    summary:
      "프로파일 표에서 일부 loci 를 빼거나, 반대로 그것만 남깁니다. 문제가 있는 유전자를 분석에서 제외할 때 씁니다.",
    needs: "AlleleCall 결과 표와 대상 loci 목록 파일",
    gives: "걸러낸 프로파일 표 하나",
  },
  JoinProfiles: {
    summary:
      "여러 번에 나눠 돌린 AlleleCall 결과를 표 하나로 합칩니다. 균주를 계속 추가하며 분석할 때 필요합니다.",
    needs: "같은 스키마로 만든 결과 표 두 개 이상",
    gives: "합쳐진 프로파일 표 하나",
    caution:
      "스키마가 자란 뒤의 결과를 예전 결과와 합칠 때는 [공통 loci 만] 을 켜야 합니다. 열 구성이 다르면 그냥은 합쳐지지 않습니다.",
  },
  SchemaEvaluator: {
    summary:
      "스키마 자체를 훑어 loci 마다 allele 이 몇 개인지, 길이가 얼마나 들쭉날쭉한지를 브라우저에서 볼 수 있는 리포트로 만듭니다. 스키마를 실제 분석에 쓰기 전에 이상한 loci 가 없는지 확인하는 용도입니다.",
    needs: "스키마 (앱 저장소에 있는 것)",
    gives: "schema_report.html — 결과 폴더로 회수되고 [리포트 열기] 로 브라우저에서 봅니다",
    caution:
      "[loci 마다 상세 페이지] 를 켜면 loci 수만큼 MAFFT 정렬을 돌립니다. loci 3,127개 기준 3초에서 39초로 늘어나고, 회수할 파일도 loci 수만큼 늘어납니다.",
  },
  AlleleCallEvaluator: {
    summary:
      "AlleleCall 결과를 균주별·loci 별로 집계하고 core genome 을 정렬해 균주 사이의 거리와 계통수(NJ 트리)까지 담은 리포트를 만듭니다. 결과를 넘기기 전에 이상한 균주가 섞이지 않았는지 보는 용도입니다.",
    needs: "AlleleCall 결과 폴더(파일이 아니라 폴더)와 그때 쓴 스키마",
    gives:
      "allelecall_report.html — 거리 행렬·존재/부재 표·cgMLST 트리가 함께 나옵니다",
    caution:
      "AlleleCall 을 [입력이 이미 CDS 입니다(--cds)] 로 돌린 결과에는 이 모듈이 필요로 하는 cds_coordinates.tsv 가 없습니다. 그런 폴더는 고를 수 없습니다.",
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
