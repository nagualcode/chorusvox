# ChorusVox DecaBuddy 🎙️🎛️

**ChorusVox DecaBuddy** é uma recriação em Rust e modernização do clássico processador de harmonia vocal de rack **Akai Professional DecaBuddy HV10** (1999) e de sua contraparte em software (`DecaBuddy.dll` v1.02).

O projeto combina as **37 tabelas harmônicas cromáticas e constantes matemáticas originais** extraídas por engenharia reversa do binário da Akai com a **modelagem física do circuito analógico e digital** documentado no Manual de Serviço do hardware HV10 (`PC MAIN BLK L8020A501A`), adicionando ainda um motor **Live de Latência Zero** e inteligência harmônica moderna para **acordes de Jazz (`FlexiChd`)**.

---

## 🍎 Download e Instalação (macOS — AU & VST3)

> **Aviso de Plataforma:** Os binários pré-compilados disponíveis na aba **[Releases](../../releases)** são exclusivos para **macOS** (compatíveis com DAWs como Ableton Live, Logic Pro, Reaper, Bitwig, Studio One, etc.).

1. Acesse a página de **[Releases](../../releases)** deste repositório.
2. Na versão mais recente, abra a aba **Assets** e baixe o arquivo **`ChorusVox-DecaBuddy-macOS-AU-VST3.zip`** *(não baixe o `Source code.zip`)*.
3. Descompacte o arquivo `.zip` e copie os bundles para as pastas de plugins do seu macOS:
   * **Audio Unit (AUv2):** Copie `ChorusVox DecaBuddy.component` para:
     `~/Library/Audio/Plug-Ins/Components/`
   * **VST3:** Copie `ChorusVox DecaBuddy.vst3` para:
     `~/Library/Audio/Plug-Ins/VST3/`
4. Reinicie a sua DAW para reescanear os plugins.

---

## 📖 Manual de Instruções — Guia Completo de Controles

A interface do **ChorusVox DecaBuddy** é organizada horizontalmente em 6 faixas independentes: **Global & Input**, **Direct & Unison** e as 4 vozes de harmonia (**VOX 1** a **VOX 4**).

---

### 1. Seção `GLOBAL & INPUT`

Controla a tonalidade da música, o comportamento de recepção MIDI, a afinação de referência, o ganho do pré-amplificador de entrada e o modo de latência da engine.

| Controle | Faixa / Opções | Função Detalhada |
| :--- | :--- | :--- |
| **Key** | `C`, `C#`, `D`, `D#`, `E`, `F`, `F#`, `G`, `G#`, `A`, `A#`, `B` | Define a tonalidade fundamental da música para todas as vozes que estiverem utilizando escalas automáticas (modais, menores harmônicas ou menores melódicas). |
| **MIDI Mode** | `Auto Ch`, `Mono (1-4)`, `Wait All`, `Priority`, `FlexiChd` | Define como as notas recebidas via MIDI controlam as vozes cuja escala (`Scale`) esteja configurada em **`MIDI`** *(veja o detalhamento dos 5 modos abaixo)*. |
| **MIDI Ch** | `1` a `16` | Define o canal MIDI base de recepção. No modo `Mono (1-4)`, as vozes 1 a 4 escutam os canais `MIDI Ch`, `MIDI Ch + 1`, `MIDI Ch + 2` e `MIDI Ch + 3`. |
| **Tune Hz** | `427 Hz` a `453 Hz` *(Padrão: `440 Hz`)* | Calibra a frequência de referência do Lá central (A4) para o detector de pitch YIN. |
| **In Gain** | `-15.0 dB` a `+15.0 dB` *(Padrão: `0.0 dB`)* | Ajusta o nível de entrada antes do estágio de saturação analógica e do detector de pitch. Aumentar o ganho empurra suavemente o circuito de entrada modelado. |
| **Live** | `On` / `Off` *(Padrão: `On`)* | **Modo Live (`On`):** O sinal **Direct** (sua voz original) passa com **0 ms de latência** (zero-latency monitoring) e as harmonias usam janela causal curta, ideal para cantar monitorando em tempo real na DAW. **Modo Studio (`Off`):** Ativa o *lookahead* simétrico completo com compensação de atraso da DAW (PDC). *Nota: Durante o Bounce/Export offline na DAW, o plugin ativa o modo Studio automaticamente.* |
| **In** *(Meter)* | Medidor Vertical | Exibe o nível de pico do sinal de entrada pós-ganho, modelado segundo a balística do circuito detector `NJU7032D` e dos 3 LEDs do painel frontal do HV10 (`-12 dB`, `-6 dB` e `0 dB/Over`). |

#### Detalhamento dos Modos MIDI (`MIDI Mode`)
Para que qualquer voz responda ao teclado MIDI, o knob **Scale** daquela voz deve estar posicionado em **`MIDI`** (tudo à esquerda). Todos os modos suportam **Pedal de Sustain (`CC 64`)**:

* **`Auto Ch` (Vanilla / Clássico):** Comportamento original determinístico da Akai. Se você tocar um acorde em um único canal MIDI (`MIDI Ch`), o plugin ordena as notas da mais grave para a mais aguda e as distribui diretamente para as vozes ativas. Se você enviar notas em canais separados (`MIDI Ch + 1`, `+ 2`, `+ 3`), ele chaveia automaticamente para alocação multicanal.
* **`Mono (1-4)`:** Cada voz harmônica (`VOX 1` a `VOX 4`) responde exclusivamente ao seu próprio canal MIDI consecutivo a partir do canal base. Ideal para escrever linhas de contraponto independentes na DAW.
* **`Wait All`:** As vozes harmônicas permanecem em silêncio até que você pressione um número de teclas MIDI igual ou superior ao número de vozes configuradas em modo `MIDI`.
* **`Priority`:** As notas do acorde MIDI são atribuídas às vozes na ordem cronológica exata em que as teclas foram pressionadas (primeiro a entrar, primeiro a ser alocado).
* **`FlexiChd` (Modern Jazz Harmony Engine):** Você segura um acorde no teclado MIDI (ex.: `Dmaj7`, `F#m6`, acordes de 9ª/13ª ou *guide tones* de 2 notas) e **as 4 harmonias se movem dinamicamente acompanhando a melodia que você canta**, escolhendo apenas notas pertencentes à harmonia daquele acorde:
  * **Inferência de *Chord-Scale*:** Combina as notas do acorde MIDI com a tonalidade (`Key`) caso você utilize *shell voicings* reduzidos.
  * **Condução de Vozes (*Voice Leading*):** Na troca entre acordes que possuem notas em comum (como `F#`, `A` e `C#` na transição `Dmaj7 -> F#m6`), o motor prioriza manter as notas comuns estacionárias para evitar saltos bruscos.
  * **Prevenção de Choques (*Anti-Clash b9*):** Impede que qualquer voz harmônica estacione a exatamente 1 semitom (`2ª menor`) da nota que você está cantando no momento.

---

### 2. Seção `DIRECT & UNISON`

Controla o sinal da voz principal (`Direct`) e a 10ª voz dedicada de dobra em uníssono (`Unison`), além do botão global `Link` e do medidor de saída estéreo.

| Controle | Faixa / Opções | Função Detalhada |
| :--- | :--- | :--- |
| **Direct** | `On` / `Off` | Liga ou desliga a presença da sua voz original limpa na mistura de saída. Desligue (`Off`) caso esteja usando o plugin em um canal de *Send/Return* (100% Wet). |
| **Dir Lev** | `-24.0 dB` a `0.0 dB` | Controla o volume da voz original (`Direct`). Em `-24.0 dB`, o sinal é completamente silenciado (`-∞`). |
| **Dir Pan** | `-50` (L) a `+50` (R) | Posiciona a voz original no panorama estéreo (`0.00` = Centro, lei de potência constante). |
| **Link** | `On` / `Off` | Quando ligado (`On`), os controles **Random**, **Formant** e **Quantise** das vozes `VOX 2`, `VOX 3` e `VOX 4` passam a seguir automaticamente os valores ajustados na `VOX 1` (mantendo a escala `Scale`, o `Level` e o `Pan` de cada voz independentes). |
| **Unison** | `On` / `Off` | Ativa a voz extra de dobra vocal em uníssono (com filtragem analógica quente em `8.5 kHz`). |
| **U.Rand** | `0.0` a `100.0` | Intensidade de variação aleatória de pitch (até `±9 cents`) e modulação de atraso (até `16 ms` + offset de `11 ms`) da voz Unison, criando efeito de *vocal doubler* natural. |
| **U.Form** | `-50.0` a `+50.0` | Altera o tamanho aparente do trato vocal da voz Unison sem mudar a sua nota musical. Valores negativos deixam o timbre mais escuro/grave; valores positivos deixam mais claro/agudo. |
| **U.Quant** | `0.0` a `100.0` | Corrige desvios de afinação na voz Unison em relação à nota cromática mais próxima. Criar um Unison com `U.Quant` alto enquanto a voz `Direct` é natural gera um efeito clássico de coro firme. |
| **U.Level** | `-24.0 dB` a `0.0 dB` | Volume de saída da voz Unison. |
| **U.Pan** | `-50` (L) a `+50` (R) | Posição estéreo da voz Unison. |
| **Out** *(Meter)* | Medidor Duplo (L/R) | Medidor de pico para os canais Esquerdo e Direito da saída mestre pós-reconstrução analógica. |

---

### 3. Seções de Harmonia: `VOX 1`, `VOX 2`, `VOX 3` e `VOX 4`

Cada uma das 4 linhas horizontais controla um gerador PSOLA independente. Quando o botão **Dual** está ativo nas 4 vozes simultaneamente (junto ao `Direct` e `Unison`), o plugin sintetiza as **10 vozes** que dão nome ao *DecaBuddy*.

| Controle | Faixa / Opções | Função Detalhada |
| :--- | :--- | :--- |
| **On** | `On` / `Off` | Ativa ou desativa a respectiva linha de voz. Possui *fade-out* suave anti-clique (~40 ms) e desliga completamente o processamento de CPU da voz durante pausas vocais ou quando inativa. |
| **Dual** | `On` / `Off` | Ativa uma segunda sub-voz gêmea para esta harmonia. A sub-voz **Dual** é automaticamente posicionada no lado oposto do panorama (`-Pan`), recebe um deslocamento adicional de formante, `-9 cents` de micro-detune, `+14 ms` de atraso de Haas e um filtro analógico *Dual Warmth* em `8.5 kHz` para dar largura estéreo sem embolar com a voz principal. |
| **Scale** | `37 Opções` | Seleciona o modo `MIDI`, a tabela `User` ou uma das 35 escalas harmônicas inteligentes da Akai: `Low Octave` (-12 st), `Low 3rd`, `Low 5th`, `Low 7th`, `Unison`, `High 3rd`, `High 5th`, `High 7th` e `High Octave` (+12 st), disponíveis nas variações `Mod 1` a `Mod 5`, `Har Min` (Menor Harmônica) e `Mel Min` (Menor Melódica). |
| **Random** | `0.0` a `100.0` | Controla o gerador estocástico de Ornstein-Uhlenbeck individual da voz, introduzindo deriva orgânica de afinação e atraso temporal para simular a imprecisão humana de um coral real. |
| **Formant** | `-50.0` a `+50.0` | Reescalona o envelope espectral (formantes da garganta) através do comprimento da janela de leitura de grão. Dica: em vozes agudas (`High 3rd` / `High 5th`), usar valores levemente negativos (`-10` a `-20`) evita o efeito "esquilo" (*chipmunk*). |
| **Quantise** | `0.0` a `100.0` | Define o quanto o desvio da sua voz cantada é compensado antes de somar o intervalo harmônico. Em `0%`, a harmonia segue fielmente o seu vibrato e glissandos; em `100%`, a harmonia trava na afinação exata da nota alvo. |
| **Level** | `-24.0 dB` a `0.0 dB` | Volume individual da voz harmônica (com compensação automática de ganho quando `Dual` é ativado). |
| **Pan** | `-50` (L) a `+50` (R) | Posição da voz principal no campo estéreo. Se `Dual` estiver ligado, a voz principal fica em `Pan` e a dobra gêmea vai automaticamente para `-Pan`. |

---

### 4. Mapa de Controladores MIDI (`MIDI CC`)

Além das notas musicais, o plugin responde aos seguintes comandos `Control Change` do hardware original:
* **`CC 64` (Sustain Pedal):** Retém as notas do acorde MIDI enquanto pressionado (`>= 64`).
* **`CC 11` (Expression):** Controla o volume Master (`-24 dB` a `0 dB`).
* **`CC 80` / `CC 81` (`>= 64`):** Carrega instantaneamente os slots de memória rápida `Preset 1` e `Preset 2`.
* **`CC 82`:** Liga/desliga o `Bypass` global (`>= 64` = Bypass ativo).
* **`CC 7`, `CC 10`, `CC 12`, `CC 13` (nos canais `MIDI Ch` a `MIDI Ch + 3`):** Controlam remotamente `Level`, `Pan`, `Formant` e `Quantise` de cada uma das 4 vozes.
* **`CC 120` / `CC 123` (All Sound / All Notes Off):** Limpa imediatamente todas as notas e estados de pedal retidos.

---

## 🔧 Fluxograma da Arquitetura e Simulação de Hardware (Akai HV10)

Para evitar a sonoridade estéril, metálica e excessivamente brilhante comum em pitch-shifters puramente digitais, o **ChorusVox** modela o caminho de sinal híbrido (analógico + DSP) extraído do **Manual de Serviço do Akai HV10 (`PC MAIN BLK L8020A501A`)** e das constantes da seção `.rdata` da `DecaBuddy.dll`:

```text
                       [ ENTRADA DE ÁUDIO (Mono L ou Estéreo L+R) ]
                                            │
                                            ▼
               ┌─────────────────────────────────────────────────────────┐
               │ 1. ESTÁGIO DE ENTRADA ANALÓGICO (2SC3330 + NJM4558L)    │
               │ • Ganho de Entrada (VR401: -15 dB a +15 dB)             │
               │ • Saturação suave simétrica livre de DC (modelando o    │
               │   headroom de fonte simples 9V DC dos Op-Amps NJM4558L  │
               │   IC12 e diodos 1SS133T-77 D1-D4)                       │
               │ • Detector balístico NJU7032D (IC401) -> Medidor In     │
               └────────────────────────────┬────────────────────────────┘
                                            │
                      ┌─────────────────────┴─────────────────────┐
                      │                                           │
         (Caminho Analógico Direct)                    (Conversão A/D PCM3003E)
         • Modo Live: 0 ms Latência                               │
         • Modo Studio: Sincronizado c/ PDC                       ▼
                      │                     ┌────────────────────────────────────────┐
                      │                     │ 2. RASTREADOR DE PITCH & ÉPOCAS GLOTAIS│
                      │                     │ • Filtro LP Adaptativo (310.96 Hz base │
                      │                     │   da DLL seguindo o 1º formante vocal) │
                      │                     │ • Pré-filtro FIR Anti-Aliasing [¼,½,¼] │
                      │                     │ • Detector YIN Vetorizado (SIMD x8)    │
                      │                     │   estendido até 45 Hz com histerese    │
                      │                     └─────────────────────┬──────────────────┘
                      │                                           │
                      │                                           ▼
                      │                     ┌────────────────────────────────────────┐
                      │                     │ 3. MOTOR TD-PSOLA (ADSP-2186M MODEL)   │
                      │                     │ • 9 Sub-Vozes Independentes:           │
                      │                     │   Unison + 4x (Main + Dual)            │
                      │                     │ • Janelamento Hann Coerente em Fase    │
                      │                     │   com proteção de pulso vizinho (1.15T)│
                      │                     │ • Deriva Ornstein-Uhlenbeck (Pitch/Dly)│
                      │                     │ • Filtro Passa-Altas Subsônico Akai    │
                      │                     │   em 32.7848 Hz (elimina boom grave)   │
                      │                     │ • Filtro DAC Akai em 12.8317 kHz       │
                      │                     │   (e 8.5 kHz exclusivo nas vozes Dual) │
                      │                     └─────────────────────┬──────────────────┘
                      │                                           │
                      └─────────────────────┬─────────────────────┘
                                            ▼
               ┌─────────────────────────────────────────────────────────┐
               │ 4. BARRAMENTO DE MIXAGEM ESTÉREO & RECONSTRUÇÃO D/A     │
               │ • Panning de Potência Constante (Main vs. -Dual)        │
               │ • Suavização exponencial por amostra (Zero Zipper)      │
               │ • Filtro de Reconstrução Analógica Burr-Brown PCM3003E  │
               │   (IC3 + Indutores LAP02TA L1/L2 + Op-Amps IC13/IC14)   │
               │   com roll-off Butterworth de 2ª ordem em 16.8 kHz      │
               │ • Soft-Clipper Assintótico de Saída (-2.5 dBFS)         │
               └────────────────────────────┬────────────────────────────┘
                                            │
                                            ▼
                              [ SAÍDA ESTÉREO FINAL (L / R) ]
```

---

## 🚀 ToDo's — Roadmap de Melhorias Futuras

### A. Realismo Sonoro e Modelagem Física de Hardware
- [ ] **Oversampling Polifásico (2x / 4x) nos Estágios Analógicos (`2SC3330` e `NJM4558L`):** Implementar sobreamostragem com filtro meia-banda linear para reintroduzir a curvatura exponencial assimétrica dos transistores bipolares `2SC3330` (`TR1`–`TR3`) e o *slew-rate limiting* de `1.0 V/µs` dos op-amps `NJM4558L` sob alimentação simples de `9V DC`, acompanhado de um bloqueador DC (`DC Blocker` de 5 Hz) antes do anel PSOLA para gerar harmônicos pares (2ª ordem) sem qualquer intermodulação nos grãos.
- [ ] **Emulação de Ponto Fixo pós-NOLA do `ADSP-2186MKST-300` (`IC1`):** Aplicar a quantização de barramento de 16 bits e arredondamento de acumulador MAC de 40 bits com *Noise Shaping* psicoacústico apenas após a divisão de normalização NOLA (fora do laço de sobreposição de grãos), resgatando a textura vintage de 16 bits do chip da Analog Devices sem elevar o piso de ruído.
- [ ] **Separação Dinâmica de Sibilantes / Consoantes (*Transient & Sibilance Bypass*):** Detectar fricativas não-vozeadas (`s`, `sh`, `t`, `k`) por taxa de cruzamento por zero (ZCR) para mantê-las em altura natural nas harmonias altas (`High 5th` e `High Octave`), evitando sibilância artificial.
- [ ] **Curva de Impedância de Microfone XLR (`4k ohms`) e Linha (`33k ohms`):** Modelar a resposta de frequência de graves/médios-graves do circuito de entrada balanceado sem transformador do HV10.

### B. Otimização Geral de CPU, Memória e Interface
- [ ] **Compactação e Intercalação dos Anéis de Saída (Cache L1):** Reduzir o tamanho dos buffers `out_ring` e `w_ring` das 9 sub-vozes de `32.768` amostras para `4.096` amostras intercaladas (`[(f32, f32); 4096]`), diminuindo o consumo de memória de ~2,3 MB para ~288 KB e mantendo todas as vozes residentes no cache L1/L2 da CPU.
- [ ] **Extração de Grão Compartilhada entre `Main` e `Dual`:** Como as sub-vozes `Main` e `Dual` de um mesmo canal leem o mesmo pulso glotal de entrada, compartilhar a leitura interpolada do `dry_ring` reduz pela metade o custo de memória das vozes dobradas.
- [ ] **Ativação Nativa de Registradores `FTZ` / `DAZ` por Bloco:** Substituir a prevenção aritmética de denormais por amostra (`+ 1e-15 - 1e-15`) pela configuração direta do registrador de hardware `FPCR` (Apple Silicon / ARM64) e `MXCSR` (x86_64) na entrada do `process()`.
- [ ] **Decimação Adaptativa para Taxa Fixa no YIN (96 kHz / 192 kHz):** Decimar o sinal de análise do detector YIN sempre para uma taxa interna constante (~`22.05 kHz`) em vez de razão fixa `/2`, garantindo que o uso de CPU permaneça baixo mesmo em sessões de `96 kHz` ou `192 kHz`.
- [ ] **Amortização (*Time-Slicing*) do YIN:** Distribuir o cálculo da função de diferença cumulativa (CMND) ao longo das amostras do *hop* para eliminar picos instantâneos de processamento em buffers de áudio ultrabaixos (`32` ou `64` amostras).
- [ ] **Interface Gráfica Customizada (`wgpu` Canvas):** Substituir o `GridLayout` declarativo padrão por um painel customizado com cores pastéis individuais para cada linha de voz (`VOX 1` a `VOX 4`), indicador visual da nota cantada vs. notas MIDI ativas e editor gráfico de 12 teclas para a escala `User`.
