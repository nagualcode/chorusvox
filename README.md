# Heaven

**Harmonizador vocal e corretor de afinação para macOS (Apple Silicon), Windows e Linux, em AU
e VST3, escrito em Rust com o framework [truce](https://github.com/truce-audio/truce).**

O Heaven põe até 4 vozes de harmonia (8 com **Double**) sobre a sua voz e soa como cantores de
verdade logo de cara, sem horas de ajustes: poucas opções, todas automatizáveis no Ableton.
Ele também corrige a afinação da voz principal, com ou sem as harmonias.

![Interface](docs/gui.png)

---

## Instalação

Baixe o pacote do seu sistema na página de [Releases](../../releases).

| Sistema | Pacote | Formatos | Onde copiar |
|---|---|---|---|
| macOS (Apple Silicon) | `Heaven-macOS-AppleSilicon.zip` | AU, VST3 | `~/Library/Audio/Plug-Ins/Components/` (AU) e `~/Library/Audio/Plug-Ins/VST3/` (VST3) |
| Windows (x64) | `Heaven-Windows-x64.zip` | VST3 | `C:\Program Files\Common Files\VST3\` |
| Linux (x64) | `Heaven-Linux-x64.tar.gz` | VST3 | `~/.vst3/` |

No macOS, se o Gatekeeper bloquear o plugin:

```bash
xattr -cr ~/Library/Audio/Plug-Ins/Components/Heaven.component ~/Library/Audio/Plug-Ins/VST3/Heaven.vst3
```

Reinicie o DAW. No Live: *Plug-Ins → Audio Units → Jardim Sonoro → Heaven* (ou *VST3*).

## Uso no Ableton Live

1. Coloque o **Heaven** como efeito na faixa de voz.
2. Escolha de onde vem a harmonia (botões no topo):
   * **CHORD**: escolha **Root** e **Chord**. Para mudar os acordes ao longo da música,
     automatize *Root* e *Chord* na timeline. A troca acontece exatamente no ponto desenhado.
     Se você antecipa o tempo forte ao cantar, a nota que começa até 80 ms antes da troca já
     usa o acorde seguinte.
   * **SCALE**: só a tonalidade (**Key** + **Scale**). As vozes andam em terças diatônicas, sem
     precisar de acordes.
   * **MIDI CHORD**: toque ou grave acordes numa faixa MIDI. Na faixa MIDI, em *MIDI To*,
     escolha a faixa de voz e depois *Heaven*. As vozes escolhem as melhores notas do acorde.
   * **MIDI NOTES**: cada nota MIDI segurada vira uma voz (até 4), na altura exata.
3. **VOICES** (1–4) e **VOICING**:
   * *Above*: harmonia fechada acima do lead.
   * *Below*: abaixo.
   * *Around*: acima e abaixo.
   * *Wide*: acorde aberto.
4. **MIX**:
   * **Harmony**: nível do grupo de vozes. Mudar o número de vozes não muda o volume.
   * **Lead**: nível da voz original. No mínimo, sai só a harmonia.
   * **Humanize**: quanto cada cantor virtual difere: atraso, deriva de afinação, timbre e nível.
   * **Width**: abertura estéreo.
   * **Character**: garganta mais longa (voz mais escura) ou mais curta (mais clara).
   * **Pure Tuning**: afina os intervalos com o lead em entonação justa, como um grupo
     a cappella.
   * **Double**: dois cantores por voz.

### Correção de afinação da voz principal

No painel **TUNE**:

* **LEAD TUNE** liga a correção. Desligada, a voz principal passa intacta.
* **RETUNE** é a velocidade. Em 0 ms a voz gruda na nota e o vibrato some (o efeito robótico).
  Por volta de 20 ms (padrão) a correção é natural e o vibrato mais lento passa. Acima disso,
  ela só acerta o desvio médio da nota.
* **AMOUNT** é quanto da correção é aplicada.
* **CHROMATIC** aceita qualquer semitom como alvo. Desligado, os alvos seguem o painel HARMONY:
  * *CHORD*: a escala do acorde atual, que pode ser automatizado na timeline;
  * *SCALE*: a tonalidade escolhida;
  * *MIDI CHORD* e *MIDI NOTES*: as notas MIDI seguradas (sem nenhuma, qualquer semitom).

**Só afinação, sem harmonias:** ligue **LEAD TUNE** e desligue **HARMONIES** no topo. Depois
escolha **SCALE**, a tonalidade e a escala. O Heaven funciona então como um auto-tune da voz
principal. Com as duas opções ligadas, as harmonias nascem da nota já corrigida.

Os formantes ficam onde estão, então o timbre da voz não muda com a correção. Bypass desliga as
duas coisas, sem cliques.

**Latência:** 80 ms, reportada ao host. O Live compensa automaticamente na reprodução e na
mixagem. Para gravar ouvindo a harmonia ao vivo o atraso é perceptível: grave a voz seca e use
o Heaven na mixagem. Os 80 ms são o preço de olhar 30 ms à frente e saber em que nota o cantor
vai chegar *antes* de escolher a harmonia.

---

## Linhagem sonora

O Heaven é herdeiro de cinco décadas de harmonizadores. Cada geração resolveu um problema e
deixou outro, e cada uma deixou uma ideia que está aqui dentro.

| Época | Marco | O que trouxe | O que ficou faltando | O que o Heaven herdou |
|---|---|---|---|---|
| 1975 | **Eventide H910 Harmonizer** | O primeiro pitch shifter digital comercial: harmonia em tempo real. | Shift por linha de atraso variável: voz de "esquilo", formantes deslocadas, glitches periódicos. | A ideia de que uma voz pode cantar com ela mesma. |
| 1989–1990 | **Lent** (*Computer Music Journal*) e **TD-PSOLA** (Moulines & Charpentier) | Pitch shift *sincronizado com o ciclo glotal*: cada grão é um ciclo da voz, então o envelope espectral (os formantes, "a cara" da voz) fica onde está. | Exige saber exatamente onde cada ciclo começa. | O motor de shift inteiro: grãos de dois períodos com janela de Hann, recolocados no período da nota alvo. |
| anos 90 | **IVL Technologies → DigiTech Vocalist** (patentes US 5,231,671 e 5,567,901) | Harmonia inteligente em hardware: o método de Lent simplificado, histerese contra o vibrato, sibilantes passando sem shift, reamostragem dos grãos para mudar o "gênero" da voz. | Notas por tabela fixa de intervalos, sem noção de acorde ou de condução de vozes. | Histerese de notas, tratamento de consoantes e controle de garganta (Character) por reamostragem do grão. |
| anos 2000 | **TC-Helicon** (VoicePrism, VoiceOne, VoiceLive) | Shift "híbrido" com correção de formantes, humanização e harmonia guiada pelo acompanhamento. Na linha das patentes de harmonia polifônica (US 8,168,877): histerese de ~0,2 semitom, nota estável só após ~100 ms, escolha entre terça menor e maior pela consonância com o acorde. | Latência mínima obrigatória em hardware ao vivo: decide a nota sem olhar à frente. | Escolha de notas por consonância com o acorde e a ideia de esperar a nota "assentar". |
| 2000s–hoje | **Antares Harmony Engine** (e Auto-Tune, US 5,973,252) | 4 vozes, *Throat Modeling*, *Naturalize*, variação de pitch e tempo, modos Chord/Scale/MIDI, Choir. | Num teste lado a lado (abaixo), harmonias que desabam para uníssono, afinação achatada e robótica, e modulação de amplitude visível no espectro. | O modelo de interação (fonte da harmonia + vozes + humanização), reduzido ao essencial. |

O que o Heaven acrescenta a essa linhagem:

* **Lookahead real:** decide a nota da harmonia sabendo onde o cantor vai chegar.
* **Transições proporcionais:** a harmonia sai da nota antiga para a nova na *mesma proporção*
  do glide do lead, e uma nota comum fica parada, como num arranjo de verdade.
* **Condução de vozes por custo:** sem segunda menor contra o lead, sem quintas paralelas,
  acorde completo, shifts curtos.
* **Afinação justa relativa ao lead:** a afinação de um grupo vocal.
* **Humanização com base em dados de coro**, não em ruído arbitrário.

---

## Como pesquisamos

O Heaven foi feito numa sessão de pesquisa e engenharia com o Claude (Anthropic), com uma
restrição curiosa: **quem escreveu o DSP não pode ouvir**. Então tudo que "soa bem" aqui
precisou primeiro *medir* bem.

### 1. Estado da arte

* **Manuais.** Lemos o *Harmony Engine User Guide* e o manual do *Harmony Engine Evo* da
  Antares de ponta a ponta. Vieram de lá a lista de ideias que funcionam (*Naturalize*,
  variação de pitch e tempo, *Throat Length*, *Glide*, *Choir*) e a organização da
  interface (fonte da harmonia → vozes → humanização).
* **Patentes.**
  * IVL/DigiTech (US 5,231,671 e US 5,567,901): janela de dois períodos, histerese "para o
    cantor poder fazer vibrato sem a harmonia oscilar", sibilantes sem shift, janela que
    encolhe em shifts para cima.
  * Harmonia a partir de acompanhamento polifônico (US 8,168,877): histerese de 0,2 semitom,
    100 ms para considerar a nota estável, terça menor contra maior decidida pelo acorde.
  * Antares (US 5,973,252): detecção de pitch por autocorrelação, a base do Auto-Tune.
* **Pesquisa acadêmica.**
  * TD-PSOLA e o método de Lent, para o shift preservando formantes.
  * YIN (de Cheveigné & Kawahara, 2002), para a detecção.
  * Estudos de Ternström sobre a dispersão de afinação em coros: ouvintes preferem 0–5 cents
    de desvio entre cantores e toleram até ~14. Medições em coros reais mostram 25–30 cents.
    Isso calibrou o **Humanize**.
* **Fóruns e produção.** Discussões no Gearspace e guias de mixagem de backing vocals
  convergem em três pontos:
  * sibilantes multiplicadas fazem harmonias parecerem "fora do tempo" e artificiais;
  * harmonias soam melhor um pouco mais finas e mais baixas que o lead;
  * excesso de uniformidade (afinação idêntica, tempo idêntico) denuncia o efeito.

  Daí as consoantes atenuadas nas harmonias, o filtro passa-altas e a diferença de timbre
  entre os cantores.

### 2. O concorrente na bancada

O Harmony Engine 4x instalado na máquina foi carregado via `pedalboard` (Spotify), com todos os
parâmetros inspecionados e renders feitos por script. Medimos a latência dele (zero: o lead
passa direto e as harmonias atrasam) e renderizamos referências com as mesmas configurações
para comparar.

### 3. Uma régua no lugar do ouvido

Montamos uma bateria de avaliação objetiva:

* **Afinação:** intervalo harmonia−lead quadro a quadro, medido com pyin.
* **Estabilidade:** jitter excedente em relação ao lead e glitches de afinação.
* **Periodicidade:** autocorrelação no período; grãos desalinhados derrubam esse valor.
* **Cliques:** transientes de alta frequência acima do envelope local.
* **Formantes:** erro de envelope medido *nos harmônicos* da saída, com a escala intencional
  de formantes descontada.
* **Espectrogramas:** inspecionados visualmente a cada mudança.

O material de teste foi o seguinte:

* vocais reais (stems de *Frederico Flores – The Bedroom Sessions* e um vocal de bossa);
* um soprano e um barítono **sintéticos com f0 conhecido**, para medir o erro do detector em
  cents absolutos.

### 4. O que a régua encontrou (e o que consertamos)

* **Soprano agudo lido uma oitava (ou um terço) abaixo.** Num sinal periódico todos os
  múltiplos do período são vales do YIN, e a lista de candidatos tinha só 8 vagas. Agora são
  32 em ordem de período, com correção de sub-harmônico genérica.
* **Voz "creaky" virando oitava abaixo.** Pulsos alternados dobram o período aparente. A
  descida de oitava passou a respeitar a continuidade, a menos que a evidência seja
  esmagadora.
* **Erro de oitava se perpetuando depois de uma transição.** Os dois vales são medíocres no
  quadro de transição. O sub-múltiplo agora só vale se for realmente periódico.
* **Leituras de 73 Hz em respirações e plosivas.** Criamos um "portão de tessitura" baseado
  na mediana recente do cantor.
* **Erro de oitava trocando a nota da harmonia.** Uma guarda dobra excursões de ±12 semitons
  de volta à oitava atual e só aceita o salto se ele persistir 120 ms.
* **Memória de condução de vozes travando o registro** (harmonia uma décima abaixo depois de
  um erro de ataque). O movimento passou a ter custo limitado, e shifts acima de uma oitava
  ficaram bem mais caros.
* **Humanização "áspera".** O processo de Ornstein-Uhlenbeck é contínuo mas não liso em
  escala curta. Uma suavização em 3 Hz levou o jitter da harmonia para *abaixo* do jitter do
  lead.
* **Caudas sopradas virando um uníssono fraco.** Criamos uma classe "vozeado fraco": a
  harmonia continua na nota, mais baixa, até a cauda morrer.

**Resultado final:** o erro mediano do detector é de 0,6 cent no soprano sintético. Depois de
descontar a escala intencional de formantes, o erro de envelope fica abaixo de 1 dB de −12 a
+12 semitons. O plugin roda a ~50× o tempo real no pior caso (4 vozes + Double a 96 kHz) e passa
no `auval` e no `pluginval` em rigor 10.

### A correção do lead em voz real

Medido em 60 s de cada um de três vocais reais, com alvos cromáticos. O desvio é a distância
até o semitom mais próximo, medida na saída. O RETUNE de 20 ms preserva o vibrato de propósito,
e por isso mantém o desvio mais alto. A coluna de nível é a faixa p1–p99 em relação ao seco, em
janelas de 10 ms.

| | desvio mediano | nível |
|---|---|---|
| voz original | 11–21 cents | |
| RETUNE 20 ms | 6–8 cents | −1,9 a +1,6 dB |
| RETUNE 0 ms | 1,3–2,4 cents | −1,9 a +1,5 dB |

Com a harmonia desligada, a correção roda a mais de 130× o tempo real.

### Lado a lado com o Harmony Engine

Mesmo trecho vocal, uma voz em terça diatônica acima:

| | Harmony Engine 4x | Heaven |
|---|---|---|
| quadros com nota errada / uníssono (0–2 semitons) | 543 | 130 |
| saltos bruscos de afinação | 27 | 2 |
| distância de envelope espectral (menor = timbre mais fiel) | 12,1 | 8,6 |
| jitter de afinação | 12,9 cents | 8,3 cents |

---

## Como funciona

* **Altura do lead:** YIN via FFT a cada 4 ms, com:
  * escolha de candidatos por continuidade;
  * testes de oitava e de sub-harmônico;
  * portão de tessitura;
  * mediana de 3 quadros.
* **Marcas de época:** uma por ciclo glotal. A primeira de cada trecho vai no pico do ciclo; as
  seguintes vêm por correlação cruzada normalizada com o ciclo anterior. Os grãos somam em
  fase: sem *phasing* nem aspereza.
* **Shifter PSOLA:**
  * janela de Hann de 2 períodos e interpolação cúbica fracionária;
  * normalização de potência pela densidade de grãos;
  * atrasos humanizados repetem ou pulam ciclos inteiros, sem efeito Doppler.
* **Correção do lead:**
  * o resultado é um deslocamento em semitons sobre o período de cada marca, então um erro de
    oitava do detector não salta a voz;
  * a nota alvo tem histerese e só troca sem chegar à vizinha depois de 150 ms, o que segura
    vibratos largos e descentrados;
  * os grãos somam normalizados pela soma das janelas, sem buracos nas emendas com consoantes;
  * os canais L e R são processados com as mesmas marcas, preservando a imagem estéreo.
* **Plano harmônico:**
  * 30 ms de lookahead e histerese contra o vibrato;
  * notas das vozes por busca exaustiva com custo de arranjo vocal;
  * contexto de acorde e MIDI com carimbo de amostra (a automação fica alinhada ao áudio).
* **Expressão:** a harmonia segue a entonação do lead (85%) e parte do vibrato (55%). Em
  scoops e glissandos segue tudo.
* **Cantores virtuais:** cada um tem atraso base, deriva lenta de afinação e tempo, nível e
  "garganta" próprios. A garganta também acompanha 10% do shift (vozes mais agudas têm tratos
  vocais mais curtos).
* **Consoantes:** −10 dB nas harmonias, sibilantes a −15 dB.
* **Mix:**
  * passa-altas em 75 Hz e um leve corte de agudos nas harmonias;
  * pan de potência constante;
  * o nível do grupo é normalizado pelo número de vozes.

## Desenvolvimento

```bash
cargo truce install --au2 --vst3            # compila, assina e instala
cargo test --no-default-features            # testes de teoria musical e do motor
cargo run --release --no-default-features --bin heaven_cli -- voz.wav saida.wav \
    mode=chord voices=3 voicing=around chords="0:Am,4:F,8:C,12:G" double=1
cargo run --release --no-default-features --bin heaven_cli -- voz.wav saida.wav \
    tune=1 harmonies=0 retune=20 mode=scale root=A scale=minor
cargo truce screenshot --out docs/gui.png
```

O `heaven_cli` roda o mesmo motor do plugin offline. Ele aceita uma linha do tempo de acordes
(como a automação do Live), notas MIDI por intervalo e `dump=arquivo.csv` com o pitch detectado
e as notas alvo de cada voz.

Para publicar uma versão, atualize `version` no `Cargo.toml` e envie uma tag:

```bash
git tag v0.2.0 && git push origin v0.2.0
```

O workflow `.github/workflows/release.yml` compila em cada sistema, valida com `pluginval`
(e `auval` no macOS) e publica os pacotes na release. Em pull requests ele gera e valida os mesmos
pacotes, sem publicar.

Licença: MIT ou Apache-2.0.
