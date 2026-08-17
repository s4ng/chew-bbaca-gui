import type { JobStatus, Module } from "../types";
import type { Messages, ModuleInfo } from "./ko";

/**
 * 화면 문자열 카탈로그 — 영어.
 *
 * `Messages` 는 `ko.ts` 가 정의한다. **여기에 키를 빠뜨리거나 함수 인자를 다르게
 * 쓰면 `tsc` 가 막는다** — 그것이 이 파일을 `Partial` 로 두지 않는 이유다.
 *
 * chewBBACA 자체 용어(loci, allele, cgMLST, schema)는 번역하지 않는다. 이 앱
 * 사용자는 chewBBACA 문서와 출력을 함께 보게 되므로, 용어를 옮기면 오히려
 * 대응이 끊긴다.
 */

const module: Record<Module, string> = {
  CreateSchema: "Create schema",
  AlleleCall: "Allele calling",
  ExtractCgMLST: "Extract core genome",
  PrepExternalSchema: "Import external schema",
  RemoveGenes: "Filter loci",
  JoinProfiles: "Join results",
  SchemaEvaluator: "Schema report",
  AlleleCallEvaluator: "Results report",
};

const moduleOption: Record<Module, string> = {
  CreateSchema: "build a wgMLST schema from assemblies",
  AlleleCall: "determine the allelic profile of each strain",
  ExtractCgMLST: "extract the core genome from allele calls",
  RemoveGenes: "filter loci out of a results table",
  JoinProfiles: "merge several results tables",
  PrepExternalSchema: "adapt and import an external schema",
  SchemaEvaluator: "schema quality report",
  AlleleCallEvaluator: "allele call quality report",
};

const moduleInfo: Record<Module, ModuleInfo> = {
  CreateSchema: {
    summary:
      "Scans the genomes of several strains and builds the list of gene positions (loci) used for comparison. Every later analysis is measured against this list, so think of it as writing the questionnaire your strains will answer.",
    needs: "A folder of assembly FASTA files",
    gives: "A schema — one FASTA file per locus. Kept in the app's store, inside WSL",
    next: "Run AlleleCall with the new schema to get a profile for each strain.",
    caution:
      "If a published schema already exists for this species, using it beats building your own — your numbers stay comparable with everyone else's.",
  },
  AlleleCall: {
    summary:
      "Decides which variant (allele) each strain carries at every locus in the schema and assigns it a number. The result is a table with one row per strain and one column per locus; how much those numbers overlap is the distance between strains.",
    needs: "An assembly folder plus the schema to use",
    gives: "results_alleles.tsv — the strain × loci profile table, copied back to your output folder",
    next: "Run ExtractCgMLST to narrow it to the core genome before comparing strains.",
    caution:
      "Sequences seen for the first time are registered as new alleles and added to the schema. A schema growing on every run is normal.",
  },
  RemoveGenes: {
    summary:
      "Drops selected loci from a profile table, or keeps only those. Use it to exclude problematic genes from an analysis.",
    needs: "An AlleleCall results table and a list of target loci",
    gives: "One filtered profile table",
  },
  JoinProfiles: {
    summary:
      "Merges AlleleCall results produced in separate runs into a single table. You need this when strains keep being added over time.",
    needs: "Two or more results tables built from the same schema",
    gives: "One merged profile table",
    caution:
      "Turn on [common loci only] when merging results from before and after the schema grew. Tables with different columns will not merge otherwise.",
  },
  SchemaEvaluator: {
    summary:
      "Walks the schema itself and reports how many alleles each locus has and how much their lengths vary, as a report you open in a browser. Use it to spot odd loci before you rely on the schema.",
    needs: "A schema (one held in the app's store)",
    gives: "schema_report.html — copied to your output folder; open it with [Open report]",
    caution:
      "Turning on [per-locus detail pages] runs one MAFFT alignment per locus. For a 3,127-locus schema that takes 39 seconds instead of 3, and produces one file per locus to copy back.",
  },
  AlleleCallEvaluator: {
    summary:
      "Aggregates AlleleCall results per strain and per locus, aligns the core genome, and reports the distances between strains along with an NJ tree. Use it to catch a strain that does not belong before you hand the results on.",
    needs: "The AlleleCall results folder (the folder, not a file) and the schema used for it",
    gives: "allelecall_report.html — distance matrix, presence/absence table and cgMLST tree together",
    caution:
      "Results produced with [input is already CDS (--cds)] have no cds_coordinates.tsv, which this module requires. Such folders cannot be selected.",
  },
  PrepExternalSchema: {
    summary:
      "Converts an existing schema into the form chewBBACA expects and imports it. If a published schema exists for your species this beats building your own — your numbers stay comparable with other people's.",
    needs: "A schema folder with one FASTA file per locus",
    gives: "The adapted schema. Used exactly like one built by CreateSchema",
    next: "Run AlleleCall with the imported schema.",
    caution:
      "If you are restoring a folder this app produced with [Export], use [Import] on the [Schemas] screen instead — that restores it as-is, with no conversion.",
  },
  ExtractCgMLST: {
    summary:
      "Picks out the loci that almost every strain has in common. The full loci table is full of gaps caused by genes present in a single strain, so it cannot compare strains fairly as it stands.",
    needs: "One results_alleles.tsv file produced by AlleleCall",
    gives:
      "A cgMLST profile table and loci list per threshold (cgMLSTschema95.txt and friends), plus a summary HTML",
    next: "Feed the resulting loci list into AlleleCall's [restrict to some loci] field and run it again to complete the cgMLST profile.",
  },
};

const step: Record<number, string> = {
  1: "1. Prepare a schema",
  2: "2. Allele calling",
  3: "3. Extract core genome",
  4: "Follow-up · checks",
};

const status: Record<JobStatus, string> = {
  queued: "Queued",
  running: "Running",
  completed: "Completed",
  failed: "Failed",
  cancelled: "Cancelled",
};

export const en: Messages = {
  dateLocale: "en-US",

  module,
  moduleOption,
  moduleInfo,
  step,
  status,

  duration: {
    hm: (h: number, m: number) => `${h}h ${m}m`,
    ms: (m: number, s: number) => `${m}m ${s}s`,
    s: (s: number) => `${s}s`,
  },

  common: {
    browse: "Browse",
    clear: "Clear",
    cancel: "Cancel",
    remove: "Delete",
    refresh: "Refresh",
    close: "Close",
    none: "(none)",
    dash: "—",
    select: "Select one",
    optional: " — optional",
  },

  app: {
    nav: {
      jobs: "Jobs",
      new: "New job",
      schemas: "Schemas",
      settings: "Settings",
    },
    checking: "Checking your environment...",
    probeFailed: (message: string) => `Environment check failed: ${message}`,
    recheck: "Check again",
    guide: "Walkthrough ↗",
    guideTitle: "A guided run through the whole pipeline with example data",
    docs: "chewBBACA docs ↗",
    distro: "Distro",
    version: (v: string) => `Version ${v}`,
  },

  jobs: {
    title: "Jobs",
    subtitle: "Run history and progress. Jobs run one at a time, in order.",
    newJob: "New job",
    adopted: (moduleLabel: string, startedAt: string) =>
      `A job started earlier is still running — ${moduleLabel} (started ${startedAt})`,
    recover: "Reattach",
    terminate: "Stop",
    empty: "No jobs yet.",
    createFirst: "Create your first job",
  },

  jobDetail: {
    back: "← Job list",
    fallbackTitle: "Job",
    openReport: "Open report",
    openOutput: "Open output folder",
    cancel: "Cancel",
    confirmCancel:
      "This cancels the running job. Work in progress will be discarded. Continue?",
    running: "In progress",
    log: "Log",
    autoScroll: "Auto-scroll",
    noOutput: "No output yet.",
    details: "Details",
    jobId: "Job ID",
    startedFinished: "Started / finished",
    exitCode: "Exit code",
    outputPath: "Output location",
    logPath: "Log file",
    args: "Arguments",
  },

  newJob: {
    title: "New job",
    subtitle:
      "Input files are copied into WSL before the run. Your originals are never modified.",
    pipelineLabel: "Typical order of the pipeline",
    needs: "Needs",
    gives: "Produces",
    nextStep: (step: number) =>
      `Next step (${step === 3 ? "back to step 2" : `step ${step + 1}`})`,
    moduleField: "Module",

    schema: "Schema",
    noSchema: "No schemas yet. Build one with CreateSchema first.",
    schemaLoci: (n: number) => ` (${n} loci)`,

    resultsDir: "AlleleCall results folder",
    resultsDirPlaceholder: "Select the results_<timestamp> folder",
    resultsDirHint:
      "Pick the folder, not a single file — several files inside are read together. Results produced with [input is already CDS] lack the required cds_coordinates.tsv, so no report can be built from them.",

    externalSchemaDir: "External schema folder",
    externalSchemaPlaceholder: "Folder containing the loci FASTA files",
    externalSchemaHint:
      "It must hold one FASTA file per locus. Select the extracted schema folder as-is.",

    joinLabel: "Results files to merge — two or more",
    pickFiles: "Choose files",
    clearFiles: "Clear",
    joinHint:
      "Choose two or more results_alleles.tsv files built from the same schema. Hold Ctrl to select several.",

    profilesFile: "AlleleCall results file",
    profilesPlaceholder: "Select results_alleles.tsv",
    profilesInvalid: (firstColumn: string, columns: number) =>
      `This is not an allelic profile table — its first column is ${firstColumn} and it has ${columns} columns.`,
    profilesInvalidHelp:
      "Select results_alleles.tsv from the AlleleCall output folder. Feeding it another TSV from the same folder (cds_coordinates.tsv and the like) makes every row count as a strain, so it runs for a long time and produces nothing useful.",
    profilesSummary: (genomes: number, loci: number) => `${genomes} strains × ${loci} loci`,
    profilesHint:
      "This is results_alleles.tsv inside the AlleleCall output folder. The module reads only that table — it never re-reads the assemblies.",

    assemblyDir: "Assembly folder",
    assemblyPlaceholder: "Select a folder",
    inputSummary: (total: number, fasta: number) => `${total} files (${fasta} look like FASTA)`,
    assemblyHint:
      "Network (UNC) paths are not supported. Please use a local drive.",

    schemaName: "Schema name",
    schemaNamePlaceholder: "e.g. Listeria monocytogenes 2026-08",
    schemaNameHint:
      "Schemas belong to the app and live inside WSL. List, delete and export them on the [Schemas] screen.",
    ptfHintCreate:
      "This training file is stored inside the schema and reused by every later AlleleCall run. It is not swapped mid-way, so results stay consistent.",

    externalNamePlaceholder: "e.g. Listeria cgMLST (Ridom)",
    externalNameHint:
      "The name shown in the list. Noting where the schema came from makes it easier to tell apart later.",
    ptfHintPrep:
      "If the external schema shipped with a training file, use that one. Supplying a different file without knowing how the schema was built will shift the CDS boundaries.",

    lociListLabel: "Restrict to some loci (--gl) — optional",
    lociListPlaceholder: "(optional) loci list text file",
    lociListFilter: "loci list",
    lociListInvalid: (tabbed: boolean) =>
      `This is not a loci list${tabbed ? " — it is a tab-separated table." : " — it is empty."}`,
    lociListInvalidHelp:
      "Select a file with one locus name per line, like the cgMLSTschema95.txt that ExtractCgMLST produces.",
    lociListSummary: (n: number) => `Will run against ${n} loci`,
    lociListHint:
      "ExtractCgMLST produces this list for you (cgMLSTschema95.txt and friends). Leave it empty to use every locus in the schema.",

    genesList: "Target loci list",
    genesListPlaceholder: "One locus name per line",
    keepInstead: "Keep only the listed loci (--inverse)",
    keepInsteadHint: "Off removes the listed loci; on keeps only them.",

    commonOnly: "Merge on common loci only (--common)",
    commonOnlyHint:
      "Turn this on when merging tables with different columns — results from before and after the schema grew are the usual case.",

    lociReports: "Also build a detail page per locus (--loci-reports)",
    lociReportsHint:
      "Lets you inspect each locus's length distribution and alignment (MSA). In exchange it runs MAFFT once per locus, so it takes far longer (3 seconds → 39 for a 3,127-locus schema) and writes one HTML file per locus into the output folder.",

    thresholds: "Presence threshold (--t) — optional",
    thresholdsPlaceholder: "Empty computes 0.95 / 0.99 / 1",
    thresholdsHint:
      "Decides which loci count as core. 0.95 means \"loci present in at least 95% of strains\". Separate several values with spaces; each value produces its own set of results.",

    cdsInput: "Input is already CDS (--cds)",
    cdsInputHint:
      "Turn this on if your FASTA holds only protein-coding sequences rather than whole genomes. It skips gene prediction (Prodigal). Getting it wrong changes the results dramatically.",

    outputDir: "Output folder",
    outputOptionalPlaceholder: "(optional) may be left empty",
    outputHintSchema:
      "The schema is kept in the app's store and managed on the [Schemas] screen. Setting this folder only leaves a copy of the run log — export the schema files with [Schemas] → [Export].",
    outputHintAlleleCall: "AlleleCall results are copied back to this folder.",
    outputHintEvaluator:
      "The HTML report is copied back to this folder. When the run finishes, open it in a browser with [Open report] on the job detail screen.",
    outputHintExtract:
      "The cgMLST profiles and loci lists (cgMLSTschema*.txt) are copied back to this folder.",

    cpu: "CPU count — optional",
    cpuDefault: (n: number) => `default: ${n}`,
    cpuAuto: "empty = automatic",
    cpuHint:
      "Leave empty to use the core count reported inside WSL. It can differ from the Windows logical core count.",

    submitting: "Submitting...",
    submit: "Run",
  },

  training: {
    label: "Prodigal training file (.trn) — optional",
    createFromDir: "Build from a folder",
    pickFile: "Choose a file",
    fileFilter: "Prodigal training file",
    intro:
      "Point at a folder of genomes for the species and the assembly with the fewest contigs is used for training. The whole folder is not used because one genome is enough for the statistics to converge, while a low-quality assembly mixed in quietly degrades the model.",
    genomeDirPlaceholder: "Folder containing genome FASTA files",
    scanning: "Scanning…",
    pickDir: "Choose folder",
    scanningHint:
      "Reading every FASTA in the folder. Contig counts cannot be derived from file size, so each file must be read — a few seconds for several hundred genomes.",
    genomeField: "Genome to train on",
    candidate: (fileName: string, contigs: number, bases: string) =>
      `${fileName} — ${contigs} contigs, ${bases}`,
    candidateHint: (scanned: number) =>
      `Of ${scanned} FASTA files, only those within the expected size range are offered. The top one is usually the right choice.`,
    nameField: "Name",
    namePlaceholder: "e.g. B_fragilis",
    nameHint:
      "Leave off the extension. If the name is already taken this fails rather than overwriting.",
    creating: "Training… (tens of seconds)",
    create: "Build",
    emptyHint:
      "Left empty, each genome is trained separately, so CDS boundaries drift slightly and needless new alleles pile up. Supply one if you plan to merge with results from elsewhere.",
  },

  schemas: {
    title: "Schemas",
    subtitle:
      "Schemas belong to the app and are stored inside WSL. AlleleCall keeps adding new alleles, so keeping them in a Windows folder would add filesystem overhead to every run.",
    importing: "Importing...",
    import: "Import",
    empty: "No schemas yet. Build one with [New job] → CreateSchema.",
    emptyHint: "If you exported a folder earlier, [Import] restores it.",
    defaultImportName: "Imported schema",
    promptName: "What should this schema be called?\n(the name shown in the list)",
    imported: (name: string, loci: number | null) =>
      `Imported '${name}'${loci ? ` (${loci} loci)` : ""}.`,
    confirmDelete: (name: string) =>
      `This deletes the schema '${name}'.\nResults already produced with it remain, but you will not be able to continue AlleleCall against the same schema.\nThis cannot be undone. Continue?`,
    exporting: "Exporting...",
    export: "Export",
    deleting: "Deleting...",
    createdAt: "Created",
    lociCount: "Loci",
    trainingFile: "Training file",
    noTrainingFile: "none",

    trainingTitle: "Prodigal training file",
    trainingSubtitle:
      "The species-specific training file used when building a schema. Build one from the training file field of [New job] → CreateSchema by pointing at a genome folder. chewBBACA ships files for only 19 species, so anything else you build yourself.",
    trainingEmpty: "No training files yet.",
    confirmDeleteTraining: (name: string) =>
      `This deletes the training file '${name}'.\nSchemas already built with it hold their own copy and are unaffected.\nThis cannot be undone. Continue?`,
    trainingCreatedAt: "Created",
    trainingSize: "Size",
  },

  settings: {
    title: "Settings",
    subtitle:
      "Only what the app owns. Your global WSL configuration (.wslconfig) is never modified.",

    envTitle: "Runtime",
    distro: "Distro",
    chewbbaca: "chewBBACA",
    cpuCount: "CPU cores",
    state: "State",
    unknown: "unknown",

    runTitle: "Execution",
    defaultCpu: "Default CPU count",
    defaultCpuPlaceholder: "empty = automatic (WSL nproc)",
    keepWorkDir: "Keep the temporary work folder after a run (for debugging)",
    saved: "Saved.",

    diskTitle: "Disk",
    diskIntro:
      "The virtual disk does not shrink on its own when files are deleted. If Windows free space does not come back after a large analysis, clean it up with the buttons below. The distro is shut down while it runs.",
    vhdx: "Virtual disk",
    pruneIntro:
      "Temporary work folders are removed automatically only for jobs that succeeded. Folders from failed or cancelled jobs stay, and an AlleleCall that stopped midway is the largest of all because it still holds intermediate files chewBBACA never got to clean up. Empty those first — only then does [Clean up disk] actually return Windows free space.",
    scan: "Scan temporary folders",
    scanning: "Scanning...",
    compact: "Clean up disk",
    compacting: "Cleaning up...",
    scanEmpty: "No temporary folders to remove. Successful jobs were already cleaned up.",
    scanFound: (count: number, size: string) =>
      `Found ${count} temporary folders totalling ${size}. Choose which to remove.`,
    onlyCopy: "Results were never copied back — this folder may be the only copy.",
    pruning: "Removing...",
    pruneButton: (count: number, size: string) => `Remove ${count} selected (${size})`,
    confirmPrune: (count: number, size: string, risky: number) =>
      `This removes ${count} temporary work folders (${size}).\n` +
      (risky > 0
        ? `${risky} of them are completed jobs whose results were never copied back — the backend folder may be the only copy.\n`
        : "") +
      "Your Windows output folders are left alone.\nThis cannot be undone. Continue?",
    pruned: (count: number, size: string) =>
      `Removed ${count} temporary folders, freeing ${size}. Press [Clean up disk] next to reclaim the Windows free space.`,
    compactedFreed: (note: string, freed: string, after: string) =>
      `${note} It shrank by ${freed}, down to ${after}.`,
    compactedSame: (note: string, after: string) =>
      `${note} The file is still ${after} — sparse mode returns space lazily, so it will shrink as the distro releases blocks.`,

    rootfsTitle: "rootfs image",
    rootfsIntro:
      "The chewBBACA image ships with the app. Leaving the field below empty is normal — fill it in only when substituting a rootfs you built yourself.",
    rootfsUrl: "File path or URL (empty uses the bundled image)",
    rootfsUrlHint:
      "A local tar.gz path is verified and registered as-is; an http(s) address is downloaded (e.g. C:\\…\\dist-rootfs\\chewie-rootfs-3.5.4.tar.gz). Anything here takes precedence over the bundled image — remember to change the checksum too.",
    rootfsShaHint: "64 hex digits. A mismatch discards the downloaded file.",

    mcpTitle: "MCP server",
    mcpIntro:
      "Lets an MCP client such as the ChatGPT desktop app read and drive this app's features. The server runs only while this app is open, and accepts connections from this PC only (127.0.0.1).",
    mcpChecking: "Checking...",
    mcpRunning: (url: string) => `Running · ${url}`,
    mcpFailed: "Failed to start (a port conflict is likely)",
    mcpOff: "Off",
    mcpEnable: "Enable the MCP server",
    mcpAllowRun: "Allow running jobs (off makes it read-only)",
    mcpAllowRunHint:
      "While on, jobs requested by a client are queued without asking again in the app. Your client may have its own tool-approval setting.",
    mcpPort: "Port",
    mcpPortHint:
      "If the port is taken, the next one is used automatically. [State] above shows the actual address.",
    mcpClientValues: "Values for your client",
    mcpClientValuesHint:
      "The ChatGPT desktop app's [Connect to custom MCP] screen has separate fields. Paste the three values below into them one by one. The type is [Streamable HTTP].",
    mcpHeaderName: "Header key",
    mcpHeaderValue: "Header value",
    mcpCopy: "Copy",
    mcpCopied: (label: string) => `Copied ${label}.`,
    mcpCopyFailed: "Could not copy. The field is selected — press Ctrl+C.",
    mcpTokenWarning:
      "[Header value] contains your token. Do not pass it on to anyone. Leave the ChatGPT form's [default token environment variable] field empty — it expects the name of an environment variable, not the token.",
    mcpConfigSummary: "For clients configured by file (Codex CLI and similar)",
    mcpConfigLabel: "the configuration",
    mcpConfigHint: "Paste it into ~/.codex/config.toml.",
    mcpOpenGuide: "How to connect",
    mcpRegenerate: "Reissue token",
    mcpGuideHint:
      "Walks through registering it with the ChatGPT desktop app, with screenshots. If you registered it but see no tools, check that the conversation is in [Work] mode first.",
    mcpRegenerateConfirm:
      "This issues a new token.\nEvery client configuration handed out so far stops working immediately and must be pasted in again.\nContinue?",
    mcpRegenerated: "A new token was issued. Paste the settings below into your client again.",

    removeTitle: "Removal",
    removeIntro:
      "Removes the dedicated distro entirely. Your other WSL distros are unaffected.",
    removeEnv: "Remove the distro",
    removeEnvConfirm:
      "This removes the dedicated distro.\nSchemas owned by the app are deleted with it. Export them from the [Schemas] screen first if you need them.\nThis cannot be undone. Continue?",
    removedEnv: "The distro was removed.",
    loading: "Loading settings...",
  },

  lang: {
    title: "Language",
    label: "Display language",
    auto: "Follow system language",
    ko: "한국어",
    en: "English",
    autoResolved: (name: string) => `Currently displaying in ${name}.`,
    backendNote:
      "Error messages and run logs produced by the backend are still Korean only.",
  },

  dataDir: {
    label: "Data folder",
    change: "Change",
    reset: "Reset to default",
    hint:
      "The virtual disk (ext4.vhdx) is created in this folder and grows to several GB as you run analyses. If your C: drive is tight, move it to another internal drive before installing. Removable drives, network drives and exFAT cannot be used.",
    confirmPick: (picked: string) =>
      `${picked}\n\nA ChewieApp folder will be created inside and used as the data folder.\nA multi-GB virtual disk goes here, and uninstalling the app deletes this folder entirely.\nContinue?`,
    confirmReset: (defaultDir: string) =>
      `This resets the data folder to its default location:\n${defaultDir}\n\nFiles in the current folder are not moved. Continue?`,
    confirmRestart: (root: string) =>
      `The data folder is now:\n${root}\n\nThe app must restart for this to take effect. Restart now?\nChoosing [Cancel] keeps the setting — it applies from the next launch.`,
    appliesNextRun: (root: string) => `${root} will be used from the next launch.`,
  },

  onboarding: {
    title: "Preparing the runtime",
    subtitle: (distro: string) =>
      `chewBBACA runs on Linux only. This app creates one dedicated WSL2 distro (${distro}) and runs everything inside it. Your existing WSL distros and global settings are left untouched.`,
    unknownGate:
      "The environment could not be assessed. Check the diagnostics below, or run scripts/check-env.bat from the project.",
    checking: "Checking...",
    recheck: "Check again",

    diagnostics: "Diagnostics",
    hypervisor: "HypervisorPresent",
    firmware: "Firmware virtualization",
    wslInstalled: "WSL installed",
    yes: "yes",
    no: "no",
    existingDistros: "Existing distros",
    noneParen: "(none)",
    vendorModel: "Vendor / model",

    step1: "① Hardware virtualization",
    step1Desc: "Checks that CPU virtualization is on and the hypervisor is running.",
    step2: "② WSL",
    step2Desc:
      "WSL2 must be installed. Installing it needs administrator rights and a reboot.",
    step3: "③ Dedicated distro",
    step3Desc: "Registers the chewBBACA image bundled with the app as a dedicated distro.",

    biosTitleOn: "Virtualization is not working",
    biosTitleOff: "CPU virtualization is turned off",
    biosFirmwareOnIntro:
      "Virtualization appears to be enabled in firmware (BIOS/UEFI), yet the hypervisor is not running. What remains is on the Windows side.",
    biosFirmwareOn1:
      "Run wsl --install --no-distribution in an administrator PowerShell and reboot. (This enables the Virtual Machine Platform feature.)",
    biosFirmwareOn2:
      "If nothing changes, run bcdedit /set hypervisorlaunchtype auto in an administrator PowerShell and reboot. That covers hypervisor launch being disabled.",
    biosFirmwareOn3:
      "Corporate security policy or other virtualization software (older VMware/VirtualBox) may also be blocking it.",
    biosFirmwareOnNote:
      "The firmware instructions below are kept for reference in case all of the above fails.",
    biosFirmwareOffIntro:
      "Running Windows 11 does not mean virtualization is on. It is not part of the minimum requirements (TPM 2.0, Secure Boot). You have to enable it in firmware (BIOS/UEFI).",
    biosStep1: "1. Go straight to firmware",
    biosStep1Desc:
      "Reboots directly into the UEFI setup — no key-mashing during boot. (Not available on legacy BIOS machines; use the manual route below.)",
    biosReboot: "Reboot into UEFI",
    biosRebootConfirm:
      "This reboots now and opens the UEFI setup screen.\nSave any unsaved work first.\nContinue?",
    biosStep2: "2. Enter it yourself",
    biosVendor: "Vendor",
    biosEntryKey: "Entry key",
    biosMenuPath: "Where to look",
    biosVendorNote:
      "The setting is named differently by each vendor. Intel calls it Intel Virtualization Technology / VT-x, AMD calls it SVM Mode.",
    biosStep3: "3. How to confirm",
    biosStep3Desc:
      "In Task Manager → Performance → CPU, [Virtualization: Enabled] means it is on. Once enabled, press [Check again] here.",

    wslTitle: "WSL needs to be installed",
    wslIntro:
      "This needs administrator rights and a reboot. The button below raises a UAC prompt; the app itself keeps running unprivileged. No other Linux distro is installed.",
    wslInstall: "Install WSL",
    wslInstalling: "Installing...",
    wslDenied:
      "Elevation was denied. You can run the commands below yourself in an administrator PowerShell.",
    wslDeniedHow: "Start → right-click \"PowerShell\" → Run as administrator",
    wslAfter:
      "When it finishes, reboot and start this app again. There is nothing to remember — it picks up where it left off.",
    copy: "Copy",
    copied: "Copied",

    distroTitleRemote: "Download the chewBBACA environment",
    distroTitleLocal: "Install the chewBBACA environment",
    distroIntro: (remote: boolean, offline: boolean) =>
      `${remote ? "Downloads and registers" : "Registers"} the image containing chewBBACA and BLAST+ / MAFFT / FastTree as a dedicated distro. This happens once.${offline ? " The image ships with the app, so no internet connection is needed." : ""}`,
    distroMissing:
      "The rootfs image bundled with the app could not be found. If you installed from the installer, please reinstall. If you are developing, put the path to a locally built tar.gz in [Settings] → rootfs image.",
    distroInstallRemote: "Download and install",
    distroInstall: "Install",
    distroDone: "The environment is ready. Entering the app shortly...",
    stageDownload: "Downloading",
    stageVerify: "Verifying checksum",
    stageImport: "Registering the distro",
    stageDone: "Done",
    stageIdle: "Preparing",

    fallbackTitle: "If you cannot set up the environment",
    fallbackIntro:
      "Some machines make this impossible — a work laptop with a BIOS password, for instance. Two options remain.",
    fallbackGalaxy:
      "Galaxy web version — usegalaxy.eu hosts chewBBACA modules (CreateSchema, AlleleCall, DownloadSchema, PrepExternalSchema) you can run in a browser. Its version may lag behind the latest.",
    fallbackViewer:
      "Results viewer mode — open HTML reports and TSVs produced on another PC in this app. (planned for v0.2)",
  },
};
