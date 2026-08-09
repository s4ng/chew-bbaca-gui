# chewie-env rootfs 생성 전용 이미지 (ARCHITECTURE.md §8.2).
#
# Docker 는 **빌드 타임에만** 쓴다. 사용자 PC 에는 Docker 가 전혀 필요하지 않다.
# 이 이미지에서 뽑아낸 tar.gz 를 `wsl --import` 로 등록하는 것이 배포 방식이다.

FROM mambaorg/micromamba:1.5-jammy

USER root

# BLAST+ / MAFFT / FastTree 는 chewbbaca 의 의존성으로 함께 들어온다.
RUN micromamba install -y -n base -c conda-forge -c bioconda \
      chewbbaca=3.5.4 && \
    micromamba clean -a -y

# WSL 배포판 설정. 기본 사용자를 지정하지 않으면 root 로 들어가고,
# interop 을 켜 둬야 앱이 필요할 때 Windows 실행 파일을 호출할 수 있다.
COPY wsl.conf /etc/wsl.conf

# micromamba 환경이 **non-login shell 에서도** 활성화되어야 한다.
# 앱은 `bash -lc` 로 실행하지만, 사용자가 직접 셸을 열었을 때도 동작해야
# 문제를 재현하고 진단할 수 있다.
COPY profile.d-chewie.sh /etc/profile.d/chewie.sh
RUN chmod 0644 /etc/profile.d/chewie.sh && \
    printf '\n. /etc/profile.d/chewie.sh\n' >> /etc/bash.bashrc

# 앱이 소유하는 디렉터리 (§5.3). 존재만 보장하고 내용은 런타임에 채운다.
RUN useradd -m -s /bin/bash chewie && \
    mkdir -p /home/chewie/work /home/chewie/schemas && \
    chown -R chewie:chewie /home/chewie

# 설치 검증 — 여기서 실패하면 이미지를 배포하지 않는다.
RUN bash -lc 'chewBBACA.py --version && makeblastdb -version && mafft --version' || \
    bash -lc 'chewBBACA.py --version'

USER chewie
WORKDIR /home/chewie
