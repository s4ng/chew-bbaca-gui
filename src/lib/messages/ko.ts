import type { JobStatus, Module } from "../types";

/**
 * 화면 문자열 카탈로그 — 한국어. **이 파일이 타입의 원본이다** (`lib/i18n.tsx` 참조).
 *
 * 새 문구는 여기 먼저 쓴다. `en.ts` 가 `Messages` 를 구현하므로 영어를 안 채우면
 * 빌드가 깨진다 — 번역이 조용히 뒤처지는 것을 막는 장치다.
 *
 * 값이 들어가는 문장은 템플릿 문법 대신 **함수로 적는다.** 인자 개수와 타입까지
 * `tsc` 가 맞춰주고, 런타임 파서가 필요 없다.
 */

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

const module: Record<Module, string> = {
  CreateSchema: "스키마 생성",
  AlleleCall: "Allele calling",
  ExtractCgMLST: "core genome 추출",
  PrepExternalSchema: "외부 스키마 들여오기",
  RemoveGenes: "loci 걸러내기",
  JoinProfiles: "결과 합치기",
  SchemaEvaluator: "스키마 리포트",
  AlleleCallEvaluator: "결과 리포트",
};

/** [새 작업] 의 모듈 선택 목록. 영문 모듈명은 그대로 두고 설명만 번역한다. */
const moduleOption: Record<Module, string> = {
  CreateSchema: "어셈블리로부터 wgMLST 스키마 생성",
  AlleleCall: "균주별 allelic profile 결정",
  ExtractCgMLST: "allele 결과에서 core genome 추출",
  RemoveGenes: "결과 표에서 loci 걸러내기",
  JoinProfiles: "결과 표 여러 개 합치기",
  PrepExternalSchema: "외부 스키마를 변환해 들여오기",
  SchemaEvaluator: "스키마 품질 리포트",
  AlleleCallEvaluator: "allele 결과 품질 리포트",
};

const moduleInfo: Record<Module, ModuleInfo> = {
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
    gives: "allelecall_report.html — 거리 행렬·존재/부재 표·cgMLST 트리가 함께 나옵니다",
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
    gives: "임계값별 cgMLST 프로파일 표와 loci 목록(cgMLSTschema95.txt 등), 그리고 요약 HTML",
    next: "나온 loci 목록을 AlleleCall 의 [일부 loci 만 대상으로] 칸에 넣어 다시 실행하면 cgMLST 프로파일이 완성됩니다.",
  },
};

/** 단계 표시에 쓸 이름. 1단계는 두 모듈이 공유하므로 중립적으로 적는다. */
const step: Record<number, string> = {
  1: "1. 스키마 준비",
  2: "2. Allele calling",
  3: "3. core genome 추출",
  4: "후처리 · 점검",
};

const status: Record<JobStatus, string> = {
  queued: "대기 중",
  running: "실행 중",
  completed: "완료",
  failed: "실패",
  cancelled: "취소됨",
};

export const ko = {
  /** `toLocaleString` 에 넘길 BCP 47 태그. 날짜·시각 표기가 이 값을 따른다. */
  dateLocale: "ko-KR",

  module,
  moduleOption,
  moduleInfo,
  step,
  status,

  duration: {
    hm: (h: number, m: number) => `${h}시간 ${m}분`,
    ms: (m: number, s: number) => `${m}분 ${s}초`,
    s: (s: number) => `${s}초`,
  },

  common: {
    browse: "찾아보기",
    clear: "지우기",
    cancel: "취소",
    remove: "삭제",
    refresh: "새로 고침",
    close: "닫기",
    none: "(없음)",
    dash: "—",
    select: "선택하세요",
    optional: " — 선택",
  },

  app: {
    nav: {
      jobs: "작업",
      new: "새 작업",
      schemas: "스키마",
      settings: "설정",
    },
    checking: "환경을 확인하는 중...",
    probeFailed: (message: string) => `환경 검사에 실패했습니다: ${message}`,
    recheck: "다시 검사",
    guide: "따라해보기 ↗",
    guideTitle: "예제 데이터로 전 과정을 따라가 보는 안내서",
    docs: "chewBBACA 공식 문서 ↗",
    distro: "배포판",
    version: (v: string) => `버전 ${v}`,
  },

  jobs: {
    title: "작업",
    subtitle: "실행 이력과 진행 상황입니다. 동시에 하나씩 순서대로 실행됩니다.",
    newJob: "새 작업",
    adopted: (moduleLabel: string, startedAt: string) =>
      `이전에 시작한 작업이 아직 실행 중입니다 — ${moduleLabel} (${startedAt} 시작)`,
    recover: "복구",
    terminate: "종료",
    empty: "아직 실행한 작업이 없습니다.",
    createFirst: "첫 작업 만들기",
  },

  jobDetail: {
    back: "← 작업 목록",
    fallbackTitle: "작업",
    openReport: "리포트 열기",
    openOutput: "결과 폴더 열기",
    cancel: "취소",
    confirmCancel: "실행 중인 작업을 취소합니다. 진행 중인 계산은 버려집니다. 계속할까요?",
    running: "진행 중",
    log: "로그",
    autoScroll: "자동 스크롤",
    noOutput: "아직 출력이 없습니다.",
    details: "상세",
    jobId: "작업 ID",
    startedFinished: "시작 / 종료",
    exitCode: "종료 코드",
    outputPath: "결과 위치",
    logPath: "로그 파일",
    args: "실행 인자",
  },

  newJob: {
    title: "새 작업",
    subtitle: "입력 파일은 실행 전에 WSL 내부로 복사됩니다. 원본은 수정되지 않습니다.",
    pipelineLabel: "일반적인 실행 순서",
    needs: "필요한 것",
    gives: "나오는 것",
    nextStep: (step: number) =>
      `다음 단계 (${step === 3 ? "2로 되돌아감" : `${step + 1}단계`})`,
    moduleField: "모듈",

    schema: "스키마",
    noSchema: "아직 스키마가 없습니다. 먼저 CreateSchema 로 스키마를 만드세요.",
    schemaLoci: (n: number) => ` (loci ${n})`,

    resultsDir: "AlleleCall 결과 폴더",
    resultsDirPlaceholder: "results_<날짜시각> 폴더를 선택하세요",
    resultsDirHint:
      "파일 하나가 아니라 폴더를 고릅니다. 그 안의 여러 파일을 함께 읽기 때문입니다. [입력이 이미 CDS 입니다] 를 켜고 돌린 결과에는 필요한 파일(cds_coordinates.tsv)이 없어 리포트를 만들 수 없습니다.",

    externalSchemaDir: "외부 스키마 폴더",
    externalSchemaPlaceholder: "loci FASTA 가 들어 있는 폴더",
    externalSchemaHint:
      "loci 하나당 FASTA 파일 하나로 되어 있어야 합니다. 압축을 푼 스키마 폴더를 그대로 고르세요.",

    joinLabel: "합칠 결과 파일 — 두 개 이상",
    pickFiles: "파일 고르기",
    clearFiles: "비우기",
    joinHint:
      "같은 스키마로 만든 results_alleles.tsv 를 두 개 이상 고르세요. Ctrl 을 누른 채 여러 개를 선택할 수 있습니다.",

    profilesFile: "AlleleCall 결과 파일",
    profilesPlaceholder: "results_alleles.tsv 를 선택하세요",
    profilesInvalid: (firstColumn: string, columns: number) =>
      `이 파일은 allelic profile 표가 아닙니다 — 첫 열이 ${firstColumn}, 열 ${columns}개입니다.`,
    profilesInvalidHelp:
      "AlleleCall 결과 폴더의 results_alleles.tsv 를 선택하세요. 같은 폴더의 다른 TSV(cds_coordinates.tsv 등)를 넣으면 각 행이 균주로 취급되어 오래 실행된 뒤 쓸모없는 결과가 나옵니다.",
    profilesSummary: (genomes: number, loci: number) => `균주 ${genomes}개 × loci ${loci}개`,
    profilesHint:
      "AlleleCall 결과 폴더 안의 results_alleles.tsv 입니다. 이 모듈은 어셈블리를 다시 읽지 않고 그 표만 봅니다.",

    assemblyDir: "어셈블리 폴더",
    assemblyPlaceholder: "폴더를 선택하세요",
    inputSummary: (total: number, fasta: number) =>
      `파일 ${total}개 (FASTA로 보이는 파일 ${fasta}개)`,
    assemblyHint: "네트워크 드라이브(UNC) 경로는 지원하지 않습니다. 로컬 드라이브를 사용하세요.",

    schemaName: "스키마 이름",
    schemaNamePlaceholder: "예: Listeria monocytogenes 2026-08",
    schemaNameHint:
      "스키마는 앱이 소유하며 WSL 내부에 저장됩니다. 목록·삭제·내보내기는 [스키마] 화면에서 할 수 있습니다.",
    ptfHintCreate:
      "이 training file 은 스키마 안에 함께 보관되고, 이후 AlleleCall 에서 계속 같은 것이 쓰입니다. 결과 일관성을 위해 중간에 바꾸지 않습니다.",

    externalNamePlaceholder: "예: Listeria cgMLST (Ridom)",
    externalNameHint:
      "목록에 표시될 이름입니다. 어디서 가져온 스키마인지 적어두면 나중에 구분하기 좋습니다.",
    ptfHintPrep:
      "외부 스키마에 함께 제공된 training file 이라면 그대로 쓰면 됩니다. 원 스키마가 무엇으로 만들어졌는지 모른 채 다른 것을 넣으면 CDS 경계가 어긋납니다.",

    lociListLabel: "일부 loci 만 대상으로 (--gl) — 선택",
    lociListPlaceholder: "(선택) loci 목록 텍스트 파일",
    lociListFilter: "loci 목록",
    lociListInvalid: (tabbed: boolean) =>
      `loci 목록 파일이 아닙니다${tabbed ? " — 탭으로 나뉜 표입니다." : " — 비어 있습니다."}`,
    lociListInvalidHelp:
      "ExtractCgMLST 가 만든 cgMLSTschema95.txt 처럼 한 줄에 loci 이름 하나만 있는 파일을 선택하세요.",
    lociListSummary: (n: number) => `loci ${n}개를 대상으로 실행합니다`,
    lociListHint:
      "이 목록은 ExtractCgMLST 가 만들어 줍니다 (cgMLSTschema95.txt 등). 비워두면 스키마의 모든 loci 를 대상으로 합니다.",

    genesList: "대상 loci 목록",
    genesListPlaceholder: "loci 이름이 한 줄에 하나씩",
    keepInstead: "목록에 있는 것만 남긴다 (--inverse)",
    keepInsteadHint: "끄면 목록의 loci 를 제거하고, 켜면 목록의 loci 만 남깁니다.",

    commonOnly: "공통 loci 만으로 합친다 (--common)",
    commonOnlyHint:
      "열 구성이 다른 표를 합칠 때 켜세요. 스키마가 자란 뒤의 결과를 예전 결과와 합치는 경우가 여기 해당합니다.",

    lociReports: "loci 마다 상세 페이지도 만든다 (--loci-reports)",
    lociReportsHint:
      "loci 하나하나의 길이 분포와 정렬(MSA)을 볼 수 있게 됩니다. 대신 loci 마다 MAFFT 를 돌리므로 훨씬 오래 걸리고(loci 3,127개 기준 3초 → 39초) 결과 폴더에 loci 수만큼 HTML 파일이 생깁니다.",

    thresholds: "존재 임계값 (--t) — 선택",
    thresholdsPlaceholder: "비우면 0.95 / 0.99 / 1 을 모두 계산",
    thresholdsHint:
      "어떤 loci 를 core 로 볼지 정하는 기준입니다. 0.95 면 \"균주의 95% 이상에 존재하는 loci\" 를 뜻합니다. 공백으로 구분해 여러 값을 넣을 수 있고, 값마다 결과 한 벌씩 나옵니다.",

    cdsInput: "입력이 이미 CDS 입니다 (--cds)",
    cdsInputHint:
      "게놈 전체가 아니라 단백질 코딩 서열만 담긴 FASTA 라면 켜세요. 유전자 예측(Prodigal)을 건너뜁니다. 잘못 켜면 결과가 크게 달라집니다.",

    outputDir: "결과 폴더",
    outputOptionalPlaceholder: "(선택) 비워두어도 됩니다",
    outputHintSchema:
      "만들어진 스키마는 앱 저장소에 보관되고 [스키마] 화면에서 관리합니다. 이 폴더를 지정하면 실행 로그 사본만 남습니다 — 스키마 파일은 [스키마] → [내보내기] 로 꺼냅니다.",
    outputHintAlleleCall: "AlleleCall 결과가 이 폴더로 회수됩니다.",
    outputHintEvaluator:
      "리포트 HTML 이 이 폴더로 회수됩니다. 다 끝나면 [작업 상세] 의 [리포트 열기] 로 브라우저에서 볼 수 있습니다.",
    outputHintExtract: "cgMLST 프로파일과 loci 목록(cgMLSTschema*.txt)이 이 폴더로 회수됩니다.",

    cpu: "CPU 개수 — 선택",
    cpuDefault: (n: number) => `기본값: ${n}`,
    cpuAuto: "비우면 자동",
    cpuHint:
      "비워두면 WSL 내부에서 확인한 코어 수를 사용합니다. Windows 논리 코어 수와 다를 수 있습니다.",

    submitting: "등록 중...",
    submit: "실행",
  },

  training: {
    label: "Prodigal training file (.trn) — 선택",
    createFromDir: "폴더에서 만들기",
    pickFile: "파일에서 고르기",
    fileFilter: "Prodigal training file",
    intro:
      "그 종의 게놈이 든 폴더를 고르면, contig 가 가장 적은 어셈블리 하나를 골라 학습시킵니다. 폴더 전체를 쓰지 않는 이유는 게놈 하나면 통계가 수렴하는 반면, 섞여 있는 저품질 어셈블리는 모델을 조용히 나쁘게 만들기 때문입니다.",
    genomeDirPlaceholder: "게놈 FASTA 가 든 폴더",
    scanning: "훑는 중…",
    pickDir: "폴더 선택",
    scanningHint:
      "폴더의 FASTA 를 모두 읽고 있습니다. contig 수는 파일 크기로 알 수 없어 전부 읽어야 합니다 — 게놈 수백 개면 몇 초 걸립니다.",
    genomeField: "학습에 쓸 게놈",
    candidate: (fileName: string, contigs: number, bases: string) =>
      `${fileName} — contig ${contigs}개, ${bases}`,
    candidateHint: (scanned: number) =>
      `FASTA ${scanned}개 중 크기가 정상 범위인 것만 후보로 올렸습니다. 보통은 맨 위 것을 그대로 쓰면 됩니다.`,
    nameField: "이름",
    namePlaceholder: "예: B_fragilis",
    nameHint: "확장자는 빼고 적습니다. 같은 이름이 이미 있으면 덮어쓰지 않고 실패합니다.",
    creating: "학습 중… (수십 초)",
    create: "만들기",
    emptyHint:
      "비워두면 게놈마다 따로 학습해 CDS 경계가 조금씩 달라지고, 불필요한 신규 allele 이 늘어납니다. 다른 곳의 결과와 합칠 계획이라면 넣는 것을 권합니다.",
  },

  schemas: {
    title: "스키마",
    subtitle:
      "스키마는 앱이 소유하며 WSL 내부에 저장됩니다. AlleleCall 이 신규 allele 을 계속 추가하기 때문에, Windows 폴더에 두면 실행할 때마다 파일시스템 오버헤드가 쌓입니다.",
    importing: "가져오는 중...",
    import: "불러오기",
    empty: "아직 스키마가 없습니다. [새 작업] → CreateSchema 로 만들 수 있습니다.",
    emptyHint: "전에 [내보내기] 로 빼둔 폴더가 있다면 [불러오기] 로 되돌릴 수 있습니다.",
    defaultImportName: "가져온 스키마",
    promptName: "이 스키마를 무엇으로 부를까요?\n(목록에 표시될 이름입니다)",
    imported: (name: string, loci: number | null) =>
      `'${name}' 를 가져왔습니다${loci ? ` (loci ${loci})` : ""}.`,
    confirmDelete: (name: string) =>
      `'${name}' 스키마를 삭제합니다.\n이 스키마로 만든 기존 결과는 남지만, 같은 스키마로 이어서 AlleleCall 을 할 수 없게 됩니다.\n되돌릴 수 없습니다. 계속할까요?`,
    exporting: "내보내는 중...",
    export: "내보내기",
    deleting: "삭제 중...",
    createdAt: "생성",
    lociCount: "loci 수",
    trainingFile: "training file",
    noTrainingFile: "없음",

    trainingTitle: "Prodigal training file",
    trainingSubtitle:
      "스키마를 만들 때 쓰는 종별 학습 파일입니다. [새 작업] → CreateSchema 의 training file 칸에서 게놈 폴더를 고르면 만들 수 있습니다. chewBBACA 가 배포하는 것은 19개 종뿐이라, 그 밖의 종은 직접 만들어야 합니다.",
    trainingEmpty: "아직 training file 이 없습니다.",
    confirmDeleteTraining: (name: string) =>
      `training file '${name}' 를 삭제합니다.\n이미 이것으로 만든 스키마는 자기 안에 사본을 갖고 있어 영향받지 않습니다.\n되돌릴 수 없습니다. 계속할까요?`,
    trainingCreatedAt: "만든 날짜",
    trainingSize: "크기",
  },

  settings: {
    title: "설정",
    subtitle: "앱이 소유한 것만 다룹니다. 전역 WSL 설정(.wslconfig)은 수정하지 않습니다.",

    envTitle: "실행 환경",
    distro: "배포판",
    chewbbaca: "chewBBACA",
    cpuCount: "CPU 코어",
    state: "상태",
    unknown: "확인 불가",

    runTitle: "실행",
    defaultCpu: "기본 CPU 개수",
    defaultCpuPlaceholder: "비우면 자동 (WSL nproc)",
    keepWorkDir: "완료 후 임시 작업 폴더를 남겨둔다 (디버깅용)",
    saved: "저장했습니다.",

    diskTitle: "디스크",
    diskIntro:
      "가상 디스크는 파일을 지워도 자동으로 줄지 않습니다. 대용량 분석 뒤 Windows 여유 공간이 돌아오지 않으면 아래 버튼으로 정리하세요. 정리 중에는 배포판이 종료됩니다.",
    vhdx: "가상 디스크",
    pruneIntro:
      "임시 작업 폴더는 성공한 작업에서만 자동으로 지워집니다. 실패하거나 취소한 작업의 폴더는 남아 있고, 중간에 멈춘 AlleleCall 은 정리되지 못한 중간 파일까지 안고 있어 가장 큽니다. 먼저 비우고 나서 [디스크 정리] 를 눌러야 Windows 여유 공간이 실제로 돌아옵니다.",
    scan: "임시 폴더 훑어보기",
    scanning: "훑어보는 중...",
    compact: "디스크 정리",
    compacting: "정리하는 중...",
    scanEmpty: "지울 임시 폴더가 없습니다. 성공한 작업은 이미 자동으로 정리되었습니다.",
    scanFound: (count: number, size: string) =>
      `임시 폴더 ${count}개, 합계 ${size} 를 찾았습니다. 지울 것을 고르세요.`,
    onlyCopy: "결과를 회수하지 않은 작업입니다 — 이 폴더가 유일한 사본일 수 있습니다.",
    pruning: "지우는 중...",
    pruneButton: (count: number, size: string) => `선택한 ${count}개 지우기 (${size})`,
    confirmPrune: (count: number, size: string, risky: number) =>
      `임시 작업 폴더 ${count}개를 지웁니다 (${size}).\n` +
      (risky > 0
        ? `이 중 ${risky}개는 결과를 회수하지 않은 완료 작업입니다 — 백엔드의 이 폴더가 유일한 사본일 수 있습니다.\n`
        : "") +
      "Windows 결과 폴더는 건드리지 않습니다.\n되돌릴 수 없습니다. 계속할까요?",
    pruned: (count: number, size: string) =>
      `임시 폴더 ${count}개를 지워 ${size} 를 비웠습니다. Windows 여유 공간을 되찾으려면 이어서 [디스크 정리] 를 누르세요.`,
    compactedFreed: (note: string, freed: string, after: string) =>
      `${note} 지금 ${freed} 가 줄어 ${after} 입니다.`,
    compactedSame: (note: string, after: string) =>
      `${note} 파일 크기는 아직 ${after} 그대로입니다 — sparse 는 지연 반납이라, 배포판이 블록을 반납하는 만큼 앞으로 줄어듭니다.`,

    rootfsTitle: "rootfs 이미지",
    rootfsIntro:
      "chewBBACA 이미지는 앱에 포함되어 배포됩니다. 아래 칸은 비워 두는 것이 정상이고, 직접 빌드한 rootfs 로 바꿔 쓸 때만 채우면 됩니다.",
    rootfsUrl: "파일 경로 또는 URL (비우면 포함된 이미지 사용)",
    rootfsUrlHint:
      "로컬 tar.gz 경로를 넣으면 그 파일을 그대로 검증해 등록하고, http(s) 주소를 넣으면 내려받습니다 (예: C:\\…\\dist-rootfs\\chewie-rootfs-3.5.4.tar.gz). 값을 넣으면 앱에 포함된 이미지 대신 이쪽을 씁니다 — 체크섬도 함께 바꿔야 합니다.",
    rootfsShaHint: "64자리 16진수. 일치하지 않으면 받은 파일을 폐기합니다.",

    mcpTitle: "MCP 서버",
    mcpIntro:
      "ChatGPT 데스크톱 앱 같은 MCP 클라이언트가 이 앱의 기능을 읽고 실행할 수 있게 합니다. 서버는 이 앱이 켜져 있는 동안에만 동작하고, 같은 PC(127.0.0.1)에서만 접속할 수 있습니다.",
    mcpChecking: "확인 중...",
    mcpRunning: (url: string) => `실행 중 · ${url}`,
    mcpFailed: "시작하지 못했습니다 (포트 충돌일 수 있습니다)",
    mcpOff: "꺼져 있음",
    mcpEnable: "MCP 서버 사용",
    mcpAllowRun: "작업 실행 허용 (끄면 읽기 전용이 됩니다)",
    mcpAllowRunHint:
      "켜 두면 클라이언트가 요청한 작업이 앱에서 다시 묻지 않고 큐에 들어갑니다. 클라이언트 쪽에도 별도의 도구 승인 설정이 있을 수 있습니다.",
    mcpPort: "포트",
    mcpPortHint: "사용 중이면 다음 포트로 자동으로 밀립니다. 위의 [상태] 에 실제 주소가 표시됩니다.",
    mcpClientValues: "클라이언트에 넣을 값",
    mcpClientValuesHint:
      "ChatGPT 데스크톱 앱의 [맞춤형 MCP에 연결] 화면은 칸이 따로 있습니다. 아래 세 값을 해당 칸에 하나씩 붙여 넣으세요. 유형은 [스트리밍 가능한 HTTP] 입니다.",
    mcpHeaderName: "헤더 키",
    mcpHeaderValue: "헤더 값",
    mcpCopy: "복사",
    mcpCopied: (label: string) => `${label} 복사했습니다.`,
    mcpCopyFailed: "복사하지 못했습니다. 칸이 선택되었으니 Ctrl+C 를 누르세요.",
    mcpTokenWarning:
      "[헤더 값] 에는 토큰이 들어 있습니다. 다른 사람에게 그대로 보내지 마세요. ChatGPT 폼의 [기본 token 환경 변수] 칸은 비워 둡니다 — 거기는 토큰이 아니라 환경 변수의 이름을 받는 자리입니다.",
    mcpConfigSummary: "설정 파일을 쓰는 클라이언트라면 (Codex CLI 등)",
    mcpConfigLabel: "설정",
    mcpConfigHint: "~/.codex/config.toml 에 붙여 넣습니다.",
    mcpOpenGuide: "연결 방법 보기",
    mcpRegenerate: "토큰 재발급",
    mcpGuideHint:
      "ChatGPT 데스크톱 앱에 등록하는 방법을 그림과 함께 설명합니다. 등록했는데 도구가 안 보인다면 대화창이 [Work] 모드인지부터 확인하세요.",
    mcpRegenerateConfirm:
      "새 토큰을 발급합니다.\n지금까지 배포한 클라이언트 설정은 즉시 접속할 수 없게 되며, 새 설정을 다시 붙여넣어야 합니다.\n계속할까요?",
    mcpRegenerated: "새 토큰을 발급했습니다. 아래 설정을 클라이언트에 다시 붙여넣으세요.",

    removeTitle: "제거",
    removeIntro:
      "전용 배포판을 통째로 제거합니다. 사용자의 다른 WSL 배포판에는 영향이 없습니다.",
    removeEnv: "배포판 제거",
    removeEnvConfirm:
      "전용 배포판을 제거합니다.\n앱이 소유한 스키마도 함께 삭제됩니다. 필요하면 먼저 [스키마] 화면에서 내보내세요.\n되돌릴 수 없습니다. 계속할까요?",
    removedEnv: "배포판을 제거했습니다.",
    loading: "설정을 불러오는 중...",
  },

  lang: {
    title: "언어",
    label: "표시 언어",
    auto: "시스템 언어 따라가기",
    ko: "한국어",
    en: "English",
    autoResolved: (name: string) => `현재 ${name} 로 표시하고 있습니다.`,
    backendNote:
      "백엔드가 만드는 오류 메시지와 실행 로그는 아직 한국어로만 나옵니다.",
  },

  dataDir: {
    label: "데이터 폴더",
    change: "변경",
    reset: "기본 위치로",
    hint:
      "가상 디스크(ext4.vhdx)가 이 폴더에 만들어지고 분석을 돌릴수록 수 GB 까지 자랍니다. C 드라이브가 좁다면 설치 전에 다른 내장 드라이브로 바꿔 두세요. 이동식·네트워크 드라이브와 exFAT 은 쓸 수 없습니다.",
    confirmPick: (picked: string) =>
      `${picked}\n\n이 폴더 안에 ChewieApp 폴더를 만들어 데이터 폴더로 씁니다.\n여기에 수 GB 짜리 가상 디스크가 만들어지고, 앱을 제거할 때 이 폴더는 통째로 지워집니다.\n계속할까요?`,
    confirmReset: (defaultDir: string) =>
      `데이터 폴더를 기본 위치로 되돌립니다:\n${defaultDir}\n\n지금 폴더에 있는 파일은 옮기지 않습니다. 계속할까요?`,
    confirmRestart: (root: string) =>
      `데이터 폴더를 여기로 바꿨습니다:\n${root}\n\n적용하려면 앱을 다시 시작해야 합니다. 지금 다시 시작할까요?\n[취소] 를 눌러도 설정은 남아 다음 실행부터 적용됩니다.`,
    appliesNextRun: (root: string) => `다음 실행부터 ${root} 를 씁니다.`,
  },

  onboarding: {
    title: "실행 환경 준비",
    subtitle: (distro: string) =>
      `chewBBACA 는 Linux 에서만 동작합니다. 이 앱은 전용 WSL2 배포판(${distro})을 하나 만들어 그 안에서 실행합니다. 기존 WSL 배포판과 전역 설정은 건드리지 않습니다.`,
    unknownGate:
      "환경을 판정하지 못했습니다. 아래 진단 정보를 확인하거나 프로젝트의 scripts/check-env.bat 를 실행해 주세요.",
    checking: "검사 중...",
    recheck: "다시 검사",

    diagnostics: "진단 정보",
    hypervisor: "HypervisorPresent",
    firmware: "펌웨어 가상화",
    wslInstalled: "WSL 설치",
    yes: "예",
    no: "아니오",
    existingDistros: "기존 배포판",
    noneParen: "(없음)",
    vendorModel: "제조사 / 모델",

    step1: "① 하드웨어 가상화",
    step1Desc: "CPU 가상화가 켜져 있고 하이퍼바이저가 동작하는지 확인합니다.",
    step2: "② WSL",
    step2Desc: "WSL2 가 설치되어 있어야 합니다. 설치에는 관리자 권한과 재부팅이 필요합니다.",
    step3: "③ 전용 배포판",
    step3Desc: "앱에 포함된 chewBBACA 이미지를 전용 배포판으로 등록합니다.",

    biosTitleOn: "가상화가 동작하지 않습니다",
    biosTitleOff: "CPU 가상화가 꺼져 있습니다",
    biosFirmwareOnIntro:
      "펌웨어(BIOS/UEFI)의 가상화는 켜져 있는 것으로 확인되는데 하이퍼바이저가 동작하지 않습니다. 남은 원인은 Windows 쪽입니다.",
    biosFirmwareOn1:
      "관리자 PowerShell 에서 wsl --install --no-distribution 을 실행하고 재부팅합니다. (Virtual Machine Platform 기능이 켜집니다.)",
    biosFirmwareOn2:
      "그래도 같다면 관리자 PowerShell 에서 bcdedit /set hypervisorlaunchtype auto 실행 후 재부팅합니다. 하이퍼바이저 기동이 꺼져 있는 경우입니다.",
    biosFirmwareOn3:
      "사내 보안 정책이나 다른 가상화 소프트웨어(VMware/VirtualBox 구버전)가 막고 있을 수도 있습니다.",
    biosFirmwareOnNote: "아래 펌웨어 안내는 위 방법이 모두 실패했을 때 확인용으로 남겨 둡니다.",
    biosFirmwareOffIntro:
      "Windows 11 이라고 해서 가상화가 켜져 있는 것은 아닙니다. 최소 요구사항(TPM 2.0, Secure Boot)에 가상화는 포함되지 않습니다. 펌웨어(BIOS/UEFI)에서 켜야 합니다.",
    biosStep1: "1. 펌웨어로 바로 들어가기",
    biosStep1Desc:
      "재부팅과 동시에 UEFI 설정으로 진입합니다. 부팅 중 키를 연타할 필요가 없습니다. (레거시 BIOS 기기에서는 동작하지 않습니다 — 아래 수동 방법을 쓰세요.)",
    biosReboot: "재부팅하고 UEFI 열기",
    biosRebootConfirm:
      "지금 재부팅하고 UEFI 설정 화면으로 들어갑니다.\n저장하지 않은 작업이 있으면 먼저 저장하세요.\n계속할까요?",
    biosStep2: "2. 직접 들어가기",
    biosVendor: "제조사",
    biosEntryKey: "진입 키",
    biosMenuPath: "설정 위치",
    biosVendorNote:
      "설정 항목 이름은 제조사마다 다릅니다. Intel 은 Intel Virtualization Technology / VT-x, AMD 는 SVM Mode 입니다.",
    biosStep3: "3. 확인 방법",
    biosStep3Desc:
      "작업 관리자 → 성능 → CPU 에서 [가상화: 사용] 이면 켜진 것입니다. 켠 뒤 이 화면에서 [다시 검사] 를 누르세요.",

    wslTitle: "WSL 설치가 필요합니다",
    wslIntro:
      "관리자 권한과 재부팅이 필요합니다. 아래 버튼을 누르면 권한 상승 창(UAC)이 뜨고, 앱 본체는 계속 일반 권한으로 남습니다. 다른 Linux 배포판은 설치하지 않습니다.",
    wslInstall: "WSL 설치",
    wslInstalling: "설치 중...",
    wslDenied:
      "권한 상승이 거부되었습니다. 아래 명령을 관리자 PowerShell 에서 직접 실행해도 됩니다.",
    wslDeniedHow: "시작 → \"PowerShell\" 우클릭 → 관리자 권한으로 실행",
    wslAfter:
      "설치가 끝나면 재부팅한 뒤 이 앱을 다시 실행하세요. 중단된 지점을 기억할 필요는 없습니다 — 다시 켜면 이어서 진행됩니다.",
    copy: "복사",
    copied: "복사됨",

    distroTitleRemote: "chewBBACA 환경 내려받기",
    distroTitleLocal: "chewBBACA 환경 설치",
    distroIntro: (remote: boolean, offline: boolean) =>
      `chewBBACA 와 BLAST+ / MAFFT / FastTree 가 들어 있는 이미지를${remote ? " 내려받아 " : " "}전용 배포판으로 등록합니다. 한 번만 하면 됩니다.${offline ? " 이미지는 앱에 포함되어 있어 인터넷 연결이 필요 없습니다." : ""}`,
    distroMissing:
      "앱에 포함된 rootfs 이미지를 찾을 수 없습니다. 인스톨러로 설치한 앱이라면 다시 설치해 주세요. 개발 중이라면 [설정] → rootfs 이미지 칸에 직접 빌드한 tar.gz 경로를 넣으면 됩니다.",
    distroInstallRemote: "내려받고 설치",
    distroInstall: "설치",
    distroDone: "환경이 준비되었습니다. 잠시 후 앱으로 들어갑니다...",
    stageDownload: "내려받는 중",
    stageVerify: "체크섬 검증",
    stageImport: "배포판 등록 중",
    stageDone: "완료",
    stageIdle: "준비 중",

    fallbackTitle: "환경을 구성할 수 없는 경우",
    fallbackIntro:
      "BIOS 에 관리자 암호가 걸린 회사 장비처럼 끝내 불가능한 경우가 있습니다. 그때는 다음 두 가지를 쓸 수 있습니다.",
    fallbackGalaxy:
      "Galaxy 웹 버전 — usegalaxy.eu 에 chewBBACA 모듈(CreateSchema, AlleleCall, DownloadSchema, PrepExternalSchema)이 등록되어 브라우저에서 실행할 수 있습니다. 다만 버전이 최신보다 뒤처질 수 있습니다.",
    fallbackViewer:
      "결과 뷰어 모드 — 다른 PC 에서 생성된 HTML 리포트와 TSV 를 이 앱으로 열람할 수 있습니다. (v0.2 예정)",
  },
};

export type Messages = typeof ko;
