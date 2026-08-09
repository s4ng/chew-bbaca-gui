# micromamba base 환경 활성화.
#
# 앱은 `wsl -d chewie-env -- bash -lc '...'` 로 실행하므로 로그인 셸 프로필이
# 읽힌다. 이 파일이 없으면 `chewBBACA.py` 를 PATH 에서 찾지 못한다.

export MAMBA_ROOT_PREFIX="${MAMBA_ROOT_PREFIX:-/opt/conda}"
export MAMBA_EXE="${MAMBA_EXE:-/bin/micromamba}"

if [ -x "$MAMBA_EXE" ]; then
    eval "$("$MAMBA_EXE" shell hook --shell bash 2>/dev/null)" || true
    micromamba activate base 2>/dev/null || true
fi

# 활성화가 실패해도 최소한 실행은 되도록 PATH 를 직접 얹어 둔다.
case ":$PATH:" in
    *":$MAMBA_ROOT_PREFIX/bin:"*) ;;
    *) export PATH="$MAMBA_ROOT_PREFIX/bin:$PATH" ;;
esac

# tty 가 아니면 파이썬 출력이 버퍼링되어 실시간 진행률이 보이지 않는다 (§6.4).
export PYTHONUNBUFFERED=1
