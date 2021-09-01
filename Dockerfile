FROM lopsided/archlinux:latest as builder

RUN pacman -Sy --noconfirm shaderc rustup gcc
WORKDIR /usr/src/wosim
COPY rust-toolchain.toml rust-toolchain.toml
RUN rustup show
COPY . .
RUN cargo build --release -p headless

FROM lopsided/archlinux:latest

ARG VULKAN_DRIVER

RUN pacman -Sy --noconfirm vulkan-icd-loader ${VULKAN_DRIVER}

COPY --from=builder /usr/src/wosim/target/release/wosim-headless /usr/local/bin/

WORKDIR /world

ENTRYPOINT ["wosim-headless"]

CMD ["serve"]
