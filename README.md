# ChorusVox (Akai DecaBuddy v1.02 / HV10 em Rust)

Recriacao limpa em Rust (Apple Silicon M-series) do gerador de harmonias multicanal **Akai DecaBuddy / HV10**:
- **10 vozes ("Deca")**: `Direct` + `Unison` + `Harmony 1..4` + `Dual 1..4` estereo.
- **37 tabelas de escalas originais** extraidas de `HARMONYDATA.cpp` (`0x021100` e `0x0211A0`) + **4 escalas de usuario** (`User 1..4`).
- **95 parametros VST** mapeados fielmente de `pars.cpp` (`0x023C40..0x024688`).
- **Motor DSP TD-PSOLA de baixa latencia** com deteccao de pitch YIN, filtro passa-baixas adaptativo (`LPF.cpp`), reamostragem independente de Formant (`-50..+50`), Quantise (`0..100%`) e Random Delay (`RANDOMDELAY.cpp`).

## Como compilar e testar a CLI

    cargo test
    cargo run --release --bin decabuddy_cli
    cargo run --release --bin decabuddy_cli -- entrada_mono.wav saida_stereo.wav

## Como instalar o framework Truce (VST3 / CLAP / AU v2 / Standalone)

    cargo install cargo-truce
    cargo truce doctor
