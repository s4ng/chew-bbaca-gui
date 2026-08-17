; NSIS 언인스톨 훅 (ARCHITECTURE.md §8.3).
;
; 언인스톨러가 기본으로 지우는 "앱 데이터" 는 BUNDLEID 경로
; (%LOCALAPPDATA%\io.github.chewbbaca.desktop) 뿐이다. 그런데 이 앱의 데이터는
; %LOCALAPPDATA%\ChewieApp 에 있고(§5.3), 진짜 용량은 그 안의 wsl\ext4.vhdx 다.
; 훅이 없으면 사용자가 [모든 데이터 삭제] 를 켜고 제거해도 수 GB 와 등록된
; chewie-env 배포판이 그대로 남는다 — 우리가 만든 것을 우리가 치우지 않는 셈이다.

; PRE 가 아니라 POST 인 이유: PREUNINSTALL 은 `CheckIfAppIsRunning` **앞**에서 돈다.
; 앱이 아직 떠 있고 작업이 실행 중일 수 있는 시점에 배포판을 unregister 하면
; 돌던 chewBBACA 프로세스가 통째로 사라진다. POST 는 앱 종료가 확인되고
; 설치 폴더까지 정리된 뒤라 안전하다.
!macro NSIS_HOOK_POSTUNINSTALL
  ; 체크박스를 켰을 때만 지운다. 끈 사용자는 스키마를 남기려는 것이다.
  ; 업데이트(/UPDATE)에서는 절대 타면 안 된다 — 버전만 올리려다 배포판이 날아간다.
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    DetailPrint "chewie-env 배포판을 제거하는 중... (수 분 걸릴 수 있습니다)"

    ; 32비트 NSIS 에서 $WINDIR\System32 는 SysWOW64 로 리다이렉트된다.
    ; wsl.exe 는 SysWOW64 에 없으므로 리다이렉션을 꺼야 찾을 수 있다.
    ${DisableX64FSRedirection}
    ; 배포판 이름은 앱의 고정값이다 (settings.rs 의 distro 기본값).
    ; 미등록 상태거나 WSL 이 없으면 실패하는데, 어차피 다음 줄에서 폴더를 지우므로 무시한다.
    nsExec::ExecToStack '"$WINDIR\System32\wsl.exe" --unregister chewie-env'
    Pop $0
    Pop $1
    ${EnableX64FSRedirection}

    ; vhdx 는 위에서 WSL 이 지운다. 여기서 남은 app.db / logs / cache 를 정리한다.
    ; 순서를 뒤집으면 배포판 등록만 남아 깨진 상태가 된다.
    SetShellVarContext current

    ; 데이터 폴더를 다른 드라이브로 옮긴 사용자가 있을 수 있다 (paths.rs §5.3).
    ; 그때 실제 폴더는 location.txt 가 가리키는 곳이고, 기본 폴더에는 그 파일만 남는다.
    ;
    ; **포인터가 없는 것이 정상이다** — 0.4.2 까지의 모든 설치본이 그 상태다.
    ; 아래 블록이 통째로 건너뛰어져도 마지막 RMDir 은 반드시 돌아야 하므로,
    ; 기본 폴더 삭제를 이 조건 안에 넣지 않는다.
    ClearErrors
    FileOpen $2 "$LOCALAPPDATA\ChewieApp\location.txt" r
    ${IfNot} ${Errors}
      FileRead $2 $3
      FileClose $2
      ; 앱은 줄바꿈 없이 한 줄만 쓴다. 그래도 손으로 고친 파일을 만날 수 있으므로
      ; 마지막 9글자가 폴더 이름과 맞을 때만 지운다 — 엉뚱한 경로를 RMDir /r 하는
      ; 사고를 막는 마지막 방어선이다 (paths.rs 의 ROOT_DIR_NAME 과 같아야 한다).
      StrCpy $4 $3 "" -9
      ${If} $4 == "ChewieApp"
        DetailPrint "데이터 폴더를 지우는 중: $3"
        RMDir /r "$3"
      ${EndIf}
    ${EndIf}

    RMDir /r "$LOCALAPPDATA\ChewieApp"
  ${EndIf}
!macroend
