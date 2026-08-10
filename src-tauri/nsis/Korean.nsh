; !! 이 파일은 BOM 없이 저장한다. Tauri 가 target\release\nsis\x64\ 로 복사하면서
; UTF-8 BOM 을 직접 붙이므로, 여기에 BOM 이 있으면 BOM 이 두 번 들어가고
; makensis 가 Invalid command: ";" 로 죽는다. (nsis\hooks.nsh 는 복사되지 않고
; 제자리에서 include 되므로 반대로 BOM 이 **있어야** 한글이 깨지지 않는다.)
; Tauri 기본 한국어 번역의 복사본. deleteAppData 한 줄만 바꿨다.
; 기본 문구("애플리케이션 데이터 삭제하기")는 이 앱에서 무엇이 지워지는지를 숨긴다 —
; 체크하면 사용자가 몇 시간에 걸쳐 만든 스키마와 수 GB짜리 vhdx 가 함께 사라진다.
; Tauri 업데이트로 새 LangString 이 추가되면 makensis 가 "정의되지 않음"으로 실패한다.
; 그때는 target\release\nsis\x64\Korean.nsh 를 다시 복사해 이 줄만 옮기면 된다.
LangString addOrReinstall ${LANG_KOREAN} "컴포넌트 추가 및 재설치"
LangString alreadyInstalled ${LANG_KOREAN} "이미 설치되어 있습니다"
LangString alreadyInstalledLong ${LANG_KOREAN} "${PRODUCTNAME} ${VERSION}이(가) 이미 설치되어 있습니다. 수행하고자 하는 작업을 선택하고 '다음'을 클릭하여 계속합니다."
LangString appRunning ${LANG_KOREAN} "{{product_name}}이(가) 실행 중입니다! 먼저 닫은 후 다시 시도하세요."
LangString appRunningOkKill ${LANG_KOREAN} "{{product_name}}이(가) 실행 중입니다!$\n'OK'를 누르면 실행 중인 프로그램을 종료합니다."
LangString chooseMaintenanceOption ${LANG_KOREAN} "수행하려는 관리 옵션을 선택합니다."
LangString choowHowToInstall ${LANG_KOREAN} "${PRODUCTNAME}의 설치 방법을 선택하세요.."
LangString createDesktop ${LANG_KOREAN} "바탕화면 바로가기 만들기"
LangString dontUninstall ${LANG_KOREAN} "제거하지 않기"
LangString dontUninstallDowngrade ${LANG_KOREAN} "제거하지 않기 (이 설치 프로그램에서는 제거하지 않고 다운그레이드할 수 없습니다.)"
LangString failedToKillApp ${LANG_KOREAN} "{{product_name}}을(를) 종료하지 못했습니다. 먼저 닫은 후 다시 시도하세요."
LangString installingWebview2 ${LANG_KOREAN} "WebView2를 설치하는 중입니다..."
LangString newerVersionInstalled ${LANG_KOREAN} "${PRODUCTNAME}의 최신 버전이 이미 설치되어 있습니다! 이전 버전을 설치하지 않는 것이 좋습니다. 이 이전 버전을 꼭 설치하려면 먼저 현재 버전을 제거하는 것이 좋습니다. 수행하려는 작업을 선택하고 '다음'을 클릭하여 계속합니다."
LangString older ${LANG_KOREAN} "구"
LangString olderOrUnknownVersionInstalled ${LANG_KOREAN} "시스템에 ${PRODUCTNAME}의 $R4 버전이 설치되어 있습니다. 설치하기 전에 현재 버전을 제거하는 것이 좋습니다. 수행하려는 작업을 선택하고 다음을 클릭하여 계속합니다."
LangString silentDowngrades ${LANG_KOREAN} "이 설치 프로그램에서는 다운그레이드가 비활성화되어 자동 설치 프로그램을 진행할 수 없습니다. 대신 그래픽 인터페이스 설치 프로그램을 사용하세요.$\n"
LangString unableToUninstall ${LANG_KOREAN} "제거할 수 없습니다!"
LangString uninstallApp ${LANG_KOREAN} "${PRODUCTNAME} 제거하기"
LangString uninstallBeforeInstalling ${LANG_KOREAN} "설치하기 전에 제거하기"
LangString unknown ${LANG_KOREAN} "알 수 없음"
LangString webview2AbortError ${LANG_KOREAN} "WebView2를 설치하지 못했습니다! WebView2가 없으면 앱을 실행할 수 없습니다. 인스톨러를 다시 시작해보세요."
LangString webview2DownloadError ${LANG_KOREAN} "오류: WebView2 다운로드를 실패하였습니다. - $0"
LangString webview2DownloadSuccess ${LANG_KOREAN} "WebView2 부트스트래퍼가 성공적으로 다운로드되었습니다."
LangString webview2Downloading ${LANG_KOREAN} "WebView2 부트스트래퍼 다운로드 중..."
LangString webview2InstallError ${LANG_KOREAN} "오류: 종료 코드 $1로 WebView2를 설치하지 못했습니다."
LangString webview2InstallSuccess ${LANG_KOREAN} "WebView2가 성공적으로 설치되었습니다."
LangString deleteAppData ${LANG_KOREAN} "모든 데이터 삭제 (스키마·chewie-env 배포판 포함)"
