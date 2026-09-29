# Onderzoek: ondersteuning voor meerdere OPTIMOD-modellen

Datum: 22 september 2026
Status: technisch onderzoek en architectuurbesluit, bijgewerkt 23 september 2026. De 5500i, 5500, 5700 FM, 5700 HD, 6300, 8500, 8600, 8700i, 9300 en 9400 hebben nu veilige read-only identificatie, AP/AS/LP-documentlezing, native verbindingen en eigen skins. Alleen de 5700i met firmware 3.0.1.20 heeft geverifieerde writes en live meters. Bijgewerkt 29 september 2026: de 5500 (1.2.8.24) en de 8700HD (1.0.2.161) hebben statisch afgeleide profielen met writes, recall en meters; zie [de statische analyse](2026-09-29-5500-8700hd-static-analysis.md).

## Conclusie

Het is haalbaar om de app uit te breiden naar de 8200, 8400, 8500, 5500, 5500i en latere OPTIMOD-modellen. Dit kan niet veilig met één universeel protocolprofiel. De modellen vallen in minstens vier technische families:

1. **8200:** eigen seriële RS-232-generatie.
2. **8400:** legacy TCP-besturing op poort 51200 met afzonderlijke UDP-metering op poort 16540.
3. **8500/5500/5500i/5700/5700i en diverse latere PC Remote-modellen:** dezelfde brede TCP/PC Remote-familie, doorgaans met poort 6201, terminalbesturing en hetzelfde soort tekstuele presetdocumenten. Ieder model en iedere firmwareversie houdt een eigen veld-, meter- en capabilityprofiel.
4. **5750/5950/Trio en opvolgers:** HTML5 Web UI-generatie. Hiervoor is een afzonderlijke HTTP/WebSocket-adapter nodig; SNMP alleen biedt geen bewezen volledige bediening.

De bestaande verbindingenlijst kan al meerdere apparaten bewaren, maar de protocolservice houdt op dit moment één actieve 5700i-sessie bij. Meerdere modellen opslaan is een schema- en adapterwijziging. Meerdere apparaten tegelijk live bedienen vereist daarnaast een echte sessiemanager met een eigen socket, meterstream, documentcache en foutenlog per apparaat.

De aanbevolen eerste nieuwe hardwaretarget is de **5500i**. De officiële PC Remote-versie is recent, de transportkenmerken liggen het dichtst bij de 5700i en het verschil in processing is goed af te bakenen: FM, stereo encoder en diversity delay, zonder de dubbele FM/HD-keten van de 5700i. Daarna volgen 5500, 5700 FM/HD en 8500. De 8400 en 8200 krijgen aparte adapters.

## Implementatieresultaat van de eerste adapterronde

Statische inspectie van de officiële hoofdprogramma's leverde concrete PC Remote-loginprefixen op: `5500i V`, `5500 V`, `5700FM V`, `5700HD V` en `8500 V`. De officiële handleidingen van 5500, 5500i, 5700 en 8500 documenteren bovendien `AP [PASSCODE]??`, `AS [PASSCODE]??` en `LP [PASSCODE]` als leescommando's. Officiële factorypresetbestanden bevestigen de documentfamilies `8300.10` voor 5500/5500i, `5700.50` voor 5700 FM/HD en `8500.40` voor 8500.

De app gebruikt deze combinatie nu voor veilige read-only sessies. Loginmodel, terminalbanner en documentfamilie moeten onderling overeenkomen. Een expliciet verkeerd gekozen model verbreekt de initialisatie vóór enige write. De nieuwe modellen delen dus framing en bewezen leescommando's, maar niet het 5700i-parameterprofiel, de meterindeling of schrijfcapabilities.

Ook de presentatie is een adapteronderdeel geworden. Elk model heeft een eigen `skin_id`, productmarkering, materiaalpalet, LCD-kleur, signaalpadlabel en metergroepering. De skin wordt uit de werkelijk gedetecteerde sessie gekozen. Onbekende identiteit gebruikt een neutrale skin en krijgt nooit 5700i-bediening.

De tweede read-only adapterronde voegt de officiële loginprefixen `6300 V`, `8600 V`, `8700i V`, `9300 V` en `9400 V` toe. Alle vijf officiële Remote-executables bevatten dezelfde AP/AS/LP-opdrachten en sessiemarkeringen; de 6300-, 8600- en 8700i-handleidingen documenteren de leesopdrachten bovendien expliciet. Uit de officiële factorypresets volgen de afzonderlijke documentfamilies `6300.50`, `8600.40`, `8700.51`, `9300.30` en `9400.30`. De app valideert deze combinaties en houdt writes, recall en meterdecoding voor alle vijf uitgeschakeld.

## Onderzoeksmethode en bewijskwaliteit

Er zijn uitsluitend primaire Orban-bronnen gebruikt:

- de [officiële downloadpagina](https://www.orban.com/downloads) en het gekoppelde [Orban-downloadarchief](https://www.orban-europe.com/downloads/);
- officiële bedieningshandleidingen;
- officiële PC Remote-installatiepakketten;
- actuele officiële product- en specificatiepagina's.

De pakketten zijn lokaal en alleen statisch onderzocht. Er zijn geen Orban-bestanden, presets, firmware-images of gedecompileerde bronnen aan deze repository toegevoegd. De vergelijking zoekt naar protocolmarkeringen en vergelijkt sets afdrukbare ASCII-reeksen van minimaal zes tekens. Dat bewijst verwantschap, maar niet dat opcodes, veldindexen, archiefversleuteling of meters identiek zijn. Volledige schrijfondersteuning blijft per model en firmware afhankelijk van fixtures, netwerk- of seriële captures en een reversibele hardwareproef.

## Modelmatrix

| Model/familie | Officiële verbinding | Bewezen Remote-functies | Relatie tot huidige kern | Nodige adapter | Aanbevolen volgorde |
|---|---|---|---|---|---|
| 8200 | RS-232 direct met null-modemkabel of compatibele modem | Alle meters, processing, preset recall/save, automation en setup | Andere generatie; geen Ethernet of TCP/IP in de 8200PC-handleiding | `serial-8200` | 6 |
| 8400 | Optionele Ethernetkaart; TCP 51200 en UDP 16540 voor meters; daarnaast serieel/modem | Volledige bediening, meters, presets en setup | Tekstdocumenten lijken verwant, maar sessie en metering wijken af | `legacy-8400-tcp-udp` | 5 |
| 8500 | Ingebouwde Ethernet; PC Remote TCP 6201; terminal TCP 23 | FM/HD, volledige bediening, meters, presets, automation en setup | Sterke verwantschap met de 5700i-kern | `pc-remote-v2` plus 8500-profielen | 4 |
| 5500 | Ethernet/serieel via TCP/IP; terminal TCP 23; PC Remote-pakket bevat standaard 6201 | FM, stereo encoder, meters, presets, automation en setup | Sterke verwantschap met de 5700i-kern | `pc-remote-v2` plus 5500-profielen | 2 |
| 5500i | Ethernet/serieel via TCP/IP; PC Remote-pakket bevat standaard 6201 | FM, stereo encoder, diversity delay, RDS, meters, presets en setup | Dichtste nieuwe target | `pc-remote-v2` plus 5500i-profielen | 1 |
| 5700 FM / 5700 HD | Afzonderlijke officiële PC Remote-pakketten; standaard 6201 zichtbaar in beide programma's | Modelafhankelijk FM of HD | Zeer sterke verwantschap, maar niet hetzelfde als 5700i | `pc-remote-v2` plus twee capabilityprofielen | 3 |
| 6300 | Officieel PC Remote-pakket; standaard 6201 zichtbaar | Digitale/TV-audiofuncties | Zelfde brede Remote-familie | Eigen profiel en UI-capabilities | 7 |
| 8600 FM/HD, 8700i | Officiële PC Remote-pakketten; standaard 6201 zichtbaar | Geavanceerde FM/HD-processing | Zelfde brede Remote-familie | Eigen profielen, meters en structuurregels | 7 |
| 9300, 9400 | TCP/IP PC Remote; officiële pakketten bevatten standaard 6201 | AM-processing, meters, presets en setup | Transport lijkt verwant, processing is fundamenteel anders | AM-capabilitylaag en eigen UI | 8 |
| 5750, 5750 HD, 5950, Trio | HTML5 Web UI; modelafhankelijk SNMP v2 en andere netwerkprotocollen | Browserbediening; firmwarebeheer via Web UI | Nieuwe generatie, geen bewezen 6201-route | `web-ui`-adapter na API/capture-onderzoek | 9 |

De volgorde is gebaseerd op hergebruik en testbaarheid, niet op productleeftijd.

## Bevindingen per generatie

### OPTIMOD 8200

De officiële [8200PC-handleiding](https://www.orban-europe.com/downloads/8200/8200PC/8200PC_Manual.pdf) beschrijft Windows 95/98, een directe verbinding tussen twee RS-232-poorten met een null-modemkabel of een Hayes/US Robotics-compatibele modem. De handleiding noemt geen Ethernet of TCP/IP. 8200PC toont alle meters, kan processing direct wijzigen, presets terugroepen en opslaan, en complete instellingen archiveren. De organizer kan een onbeperkte lijst OPTIMODs bewaren, terwijl de gebruikersinterface één verbonden apparaat bedient.

Gevolgen voor de app:

- voeg een seriële endpointsoort toe naast IP-adressen;
- gebruik een exclusieve seriële lock en configureer baudrate, parity, databits, stopbits en flow control vanuit een modelprofiel;
- leg login, berichtgrenzen, meterrecords en foutafhandeling opnieuw vast met een echte 8200 of een seriële capture;
- behandel een externe serial-to-IP-converter als transportbrug voor hetzelfde 8200-protocol, nooit als een 6201-apparaat;
- bouw pas schrijfacties nadat read-only identificatie, meters en een reversibele instelling zijn geverifieerd.

### OPTIMOD 8400

De officiële [8400 3.0.5-handleiding](https://www.orban-europe.com/downloads/8400/Documentation/8400_3.0.5_Operating_Manual.pdf) beschrijft een optionele Ethernet-PC Card en noemt expliciet TCP-poort **51200** en UDP-poort **16540**. UDP 16540 levert de PC Remote-metering en moet door een firewall worden doorgelaten. De Remote-app communiceert via TCP/IP, ook wanneer Windows de fysieke seriële of modemverbinding als netwerkverbinding aanbiedt.

Het officiële installatiepakket bevat leesbare `.orb`-factorypresets met onder meer `OptimodVersion=<00.08>`, `Preset Name=<...>` en regels in de vorm `C:<veld>Type:waarde;D:index;`. Dit is een voorloper van de parser die voor de 5700i wordt gebruikt. De Remote-binary bevat echter niet de latere `connect ok`-loginmarkeringen en lijkt technisch veel minder op de 8500/5500/5700i-familie.

Gevolgen voor de app:

- aparte TCP-commandosessie en UDP-meterontvanger;
- lokale netwerk- en firewallfouten afzonderlijk tonen: besturing kan werken terwijl meters ontbreken;
- een eigen documentdialect toestaan, inclusief `00.08` en het 8400-veldprofiel;
- poort 6201, terminalpoort 23, 112 meterbytes en de 5700i-opcodes nooit als standaard aannemen;
- presetconversie alleen aanbieden wanneer een expliciete converter de niet-ondersteunde velden rapporteert.

### OPTIMOD 8500

De officiële [8500 3.0.2-handleiding](https://www.orban-europe.com/downloads/8500/8500HD/Documentation/8500_3.0.2_Operating_Manual.pdf) beschrijft ingebouwde 100 Mbps Ethernet, PC Remote via TCP/IP en standaardpoort **6201**. De terminalinterface gebruikt standaard TCP **23**. PC Remote kan vele 8500-adressen bewaren maar bestuurt er één tegelijk. De processor heeft afzonderlijke FM- en digitale-radiofuncties en dus een eigen capability- en meterprofiel.

Het officiële PC Remote-programma bevat dezelfde herkenbare login- en documentmarkeringen als de huidige 5700i-kern: `connect ok`, mislukte-wachtwoordmeldingen, `connected 12345678`, `OptimodVersion=<` en `End Preset<end>`. Dit ondersteunt hergebruik van framing, login en algemene documentparsing. Het bewijst niet dat alle opcodes, meterrecords, veldindexen of presettransacties gelijk zijn.

### OPTIMOD 5500 en 5500i

De officiële [5500-handleiding](https://www.orban-europe.com/downloads/5500/Documentation/5500_1.2_Operating_Manual.pdf) en [5500i-specificaties](https://www.orban.com/specification-optimodfm5500i) beschrijven TCP/IP via Ethernet en via PPP over direct-serieel/modem. De 5500-handleiding noemt terminalpoort 23. In beide officiële PC Remote-programma's staat 6201 als standaardverbindingswaarde.

De [officiële 5500i-productbeschrijving](https://www.orban.com/in-depth-optimod5500) bevestigt dat PC Remote alle functies kan bedienen, veel apparaten in een organizer kan bewaren en presets, automation en system setup kan archiveren en herstellen. De 5500i kan 8300-, 8400- en 8500-LL-presets importeren. Niet-ondersteunde functies worden daarbij zo goed mogelijk geïnterpreteerd. Dat is expliciet conversiegedrag en geen bewijs dat volledige parameterprofielen uitwisselbaar zijn.

Belangrijke UI-capabilities:

- 5500/5500i zijn FM-processors en hebben geen 5700i-achtige dubbele FM/HD-editor;
- ze hebben wel stand-alone stereo-encodermodi;
- 5500i heeft diversity delay voor HD Radio/DAB+-installaties, maar dat is geen tweede HD-processingketen;
- structuurwissels kunnen hoorbare gevolgen hebben; de officiële 5500i-informatie noemt ongeveer twee seconden mute bij wisselen naar of van Ultra-Low-Latency Five-Band.

### OPTIMOD 5700 FM, 5700 HD en 5700i

Het [officiële 5700-archief](https://www.orban-europe.com/downloads/5700/) publiceert afzonderlijke 5700 FM- en 5700 HD-pakketten. Deze moeten als afzonderlijke capabilityprofielen worden behandeld. De huidige app is uitsluitend gebaseerd op 5700i firmware 3.0.1.20 en documentfamilie 5700.51.

Orban beschrijft op de [5700i-productpagina](https://www.orban.com/overview-optimodfm5700i) dat de Windows Remote veel 5700i's in een TCP/IP-netwerk kan beheren en dat presets van 8500, 8400, 8300, 5500 en 5300 kunnen worden geïmporteerd. Opnieuw geldt: presetimport bewijst geen identiek live-controlprotocol.

### 6300, 8600, 8700i, 9300 en 9400

Het officiële downloadarchief bevat afzonderlijke PC Remote-pakketten voor deze modellen. Statische inspectie toont de concrete loginprefixen `6300 V`, `8600 V`, `8700i V`, `9300 V` en `9400 V`, dezelfde login- en documentmarkeringen, AP/AS/LP-opdrachten en de standaardwaarde 6201. Officiële factorypresets bevestigen documentfamilies `6300.50`, `8600.40`, `8700.51`, `9300.30` en `9400.30`. Dit onderbouwt hergebruik van de begrensde sessie-, framing- en leesmodules; het onderbouwt geen parameterwrites of meterindelingen. De app heeft daarom read-only adapters met eigen profielen en skins:

- 6300: digitale/TV-audiofuncties;
- 8600/8700i: andere FM/HD-structuren, meters en optionele hardware;
- 9300/9400: AM-processing en dus een andere editor- en metercapabilityset.

Een opgeslagen model wordt altijd opnieuw gecontroleerd tegen de live PC Remote-login en terminalbanner. Een verkeerde combinatie of documentfamilie faalt gesloten. Meterputten blijven leeg zolang er geen geverifieerde meterbinding bestaat.

De [officiële 9400/9300-specificatiepagina](https://www.orban.com/specifications-optimodam9300) bevestigt TCP/IP PC Remote over Ethernet of PPP-serieel en SNMP v2. SNMP biedt volgens Orban slechts een beperkt aantal lees- en schrijfobjecten en vervangt volledige PC Remote-bediening dus niet.

### 5750, 5950 en Trio

De [5750](https://www.orban.com/overview-optimod-5750-audio-processor), [5750 HD](https://www.orban.com/optimod-5750-hd-overview), [5950](https://www.orban.com/overview-optimod-5950-audio-processor), [5950 HD](https://www.orban.com/optimod-5950-hd-overview) en [Trio](https://www.orban.com/overview-optimod-trio-audio-processor) worden officieel via iedere HTML5-browser bediend. Ook de firmware-updateprocedure loopt via de Web UI. Deze apparaten horen daarom in een afzonderlijke adapterfamilie. Zonder openbare volledige API-documentatie moet ondersteuning beginnen met read-only inspectie van de browserrequests tegen eigen hardware of met door Orban verstrekte ontwikkelaarsdocumentatie. Het onderscheppen van eigen geautoriseerd verkeer is bruikbaar voor interoperabiliteit; credentials, sessiecookies en stationdata mogen nooit in fixtures terechtkomen.

## Analyse van officiële PC Remote-pakketten

De onderstaande tabel gebruikt de hoofd-EXE uit ieder officieel pakket. `Overeenkomst` is Jaccard-overlap van unieke afdrukbare ASCII-reeksen van minimaal zes tekens met `5700iPC.exe`. Het is een heuristiek voor gedeelde code en teksten, geen protocolbewijs.

| Model | Officieel pakket | EXE SHA-256 | Overeenkomst met 5700i | Latere login/documentmarkeringen |
|---|---|---|---:|---|
| 8400 | 3.0 PC Remote | `92061d22d24ed5c3e023db55433a2149b003b7bb12120db648dd4fca30c125b3` | 0,115 | Alleen documentmarker |
| 8500 HD | 3.0.2.97 | `4cd54c6bc9a6cc3d8c0702ad63f866455c943bbeba920c2d3e19962d8709e7ee` | 0,656 | Ja |
| 5500 | 1.2.8.24 | `c8f138357e9b8758af16e1c756e88258cecb41b762b9daf7112b66649565a6a1` | 0,579 | Ja |
| 5500i | 3.1.0.2 | `d702cc2d27178a87e63e67bc88c54ee690279b4b31b68b5c272b9edea88dda72` | 0,598 | Ja |
| 5700 FM | 1.1.1.1 | `46636ebe2e06cafcc6e003f7bba174ef461354e50edf860bc65afa506bb547c0` | 0,720 | Ja |
| 5700 HD | 1.1.1.2 | `855789903c5468657e3e7c4379e0d25ed8a916229738f16504020924bff04eb6` | 0,717 | Ja |
| 5700i | 3.0.1.20 | `63def9bbe3ea4cc5714b958a330f511e9f8540b973f0cedaf596dd7956054b43` | 1,000 | Ja |
| 6300 | 4.1.1.19 | `492867138ebe47559890dc7e45237993cad45cd0dce4f5130b4757c7814369cf` | 0,506 | Ja |
| 8600 HD | 4.5.5.2 | `aa46340caf54e8a293715b27bd0178b1a259e185ef8e2c794ec854cf9f428df9` | 0,718 | Ja |
| 8700i | 1.5.1.11 | `bd1fdde1849217ff7830024c8ea2a369438a8ab6d5fd96aabfec74063df7c851` | 0,676 | Ja |
| 9300 | 2.1.1.7 | `78b041ab7946922f24d24b029fa91668efc2b5a8f6205d9d51f5601d315b0cd2` | 0,510 | Ja |
| 9400 | 2.0.1.4 | `dadf924cb2be1f0d95633c40195313541ab6939712919f266109eb8dbf21b739` | 0,575 | Ja |

De officiële pakketten zijn alleen als tijdelijke onderzoeksinput gebruikt. Hun SHA-256 is vastgelegd zodat een vervolgonderzoek exact kan controleren welke binary is onderzocht.

## Waarom de code op 22 september niet modelneutraal was

Deze lijst beschrijft de stand bij de start van dit onderzoek. Sindsdien zijn deze grenzen per model gemaakt: de adapterregistry kiest banner, documentfamilie, meterprofiel (de 5700i blijft exact 112 waarden; statisch afgeleide adapters accepteren 1–512) en parameterprofiel per exact model en firmware. De implementatie bevatte toen correcte maar harde 5700i-grenzen:

- `crates/orban-protocol/src/session.rs` accepteert alleen een firmwaretekst die met `5700i V ` begint;
- `crates/orban-protocol/src/terminal.rs` vereist een 5700i-terminalbanner;
- `crates/orban-protocol/src/document.rs` vereist `OptimodVersion=<5700.`;
- `crates/orban-protocol/src/message.rs` accepteert alleen meterbank 1 of 2 met exact 112 bytes;
- `crates/orban-protocol/src/profile.rs` embedt alleen `profiles/5700i/3.0.1.20/parameters.json`;
- `crates/orban-web/src/main.rs` bezit één `Option<Device>` en één globaal profiel;
- `crates/orban-web/src/devices.rs` bewaart host en poorten, maar geen model, adapter, transportsoort of geverifieerde firmware;
- de UI veronderstelt de 5700i-meterindeling en de FM/HD-capabilities.

Alleen de modelcontrole verwijderen zou de app gevaarlijk maken: een geldig veldnummer op de 5700i kan op een ander model een andere functie betekenen.

## Vereiste architectuur

### Adapterregister

Voeg één register toe waarin een gedetecteerd apparaat aan een expliciete adapter wordt gekoppeld. Een adapter bevat:

- model- en firmwarematchers;
- transportsoort en standaardendpoints;
- sessie/logincodec;
- framing en toegestane opcodes;
- terminal- of beheerprotocol;
- documentdialect en archiefformaat;
- parameterprofiel per firmware;
- meterrecordlengtes, kanaalbindings en displaycurves;
- preset-, setup-, automation- en routingcapabilities;
- UI-capabilities zoals FM, HD, AM, stereo encoder, diversity delay en aantal banden;
- een schrijfbeleid dat bij onbekende firmware standaard uit staat.

Een geschikte opsplitsing is:

```text
DeviceAdapter
├── IdentificationProfile
├── TransportProfile
├── SessionCodec
├── DocumentDialect
├── ParameterProfile
├── MeterProfile
├── PresetCapabilities
└── UiCapabilities
```

De bestaande 5700i-code wordt eerst als adapter ingekapseld zonder gedrag te veranderen. Pas wanneer alle bestaande tests blijven slagen, wordt een tweede model toegevoegd.

### Opgeslagen verbindingen versie 2

Breid een verbinding uit met:

```text
model_mode: auto | explicit
detected_model: optioneel
detected_firmware: optioneel
adapter_id: optioneel, alleen cache
transport: ethernet | serial | serial_bridge
control_endpoint: host/poort of serial device
meter_endpoint: optioneel apart endpoint
terminal_endpoint: optioneel
last_verified_profile: optioneel
```

Maak de migratie van versie 1 deterministisch: bestaande verbindingen worden `auto`, Ethernet, poort 6201 en terminalpoort 23. Een gecachte detectie mag nooit schrijfrechten verlenen; de actieve sessie moet model en firmware opnieuw bevestigen.

### Eén actief apparaat tegenover gelijktijdige apparaten

Er zijn twee onafhankelijke productfuncties:

1. **Meerdere opgeslagen apparaten:** verbinden, verbreken en wisselen tussen apparaten. Dit ligt dicht bij het gedrag van de originele 8500/5500/5500i Remote-apps en kan met één actieve sessie blijven werken.
2. **Meerdere apparaten tegelijk:** meters van meerdere processors tegelijk tonen of snel wisselen zonder opnieuw in te loggen. Hiervoor moet `Option<Device>` veranderen in een `HashMap<DeviceId, DeviceRuntime>`, waarbij iedere runtime een eigen owner-task, status, meterchannel, documentcache, mutatiequeue en foutenlog heeft.

Begin met meerdere modellen en één actieve sessie. Voeg gelijktijdige sessies pas toe nadat twee apparaten langdurig stabiel naast elkaar zijn getest. Dit houdt preset recall en schrijfacties per apparaat ondubbelzinnig.

### Detectie en Network

Network krijgt een gelaagde, credentialvrije scan:

1. bestaande 5700i/8500-familie terminalbanners op bekende terminalpoorten;
2. PC Remote-endpoints 6201 en 8400-endpoint 51200, uitsluitend met veilige identificatie als die per adapter is bewezen;
3. HTTP/HTTPS-identificatie voor de HTML5-generatie;
4. seriële poorten alleen na expliciete selectie door de gebruiker;
5. mDNS/Bonjour uitsluitend wanneer een officieel apparaat werkelijk een service adverteert.

Detectie mag geen toegangscode versturen en geen sessie achterlaten. Een open poort zonder geverifieerde modelidentiteit wordt als onbekend endpoint getoond, niet automatisch als OPTIMOD toegevoegd.

### Profielen en fixtures

Gebruik per model en firmware een eigen map:

```text
profiles/<model>/<firmware>/parameters.json
profiles/<model>/<firmware>/meters.json
profiles/<model>/<firmware>/capabilities.json
profiles/<model>/<firmware>/fixtures/*.bin
```

Fixtures bevatten geschoonde protocolrecords en synthetische credentials. Sla nooit live toegangscodes, IP-adressen, stationsnamen, user presets of complete on-air documenten op.

### Presets

Een generiek presetbestand krijgt minimaal:

- bronmodel en bronfirmware;
- documentdialectversie;
- profile-hash;
- type: factory-reference, user, modified of lokale export;
- expliciete compatibiliteitsregels;
- lijst met overgeslagen of geconverteerde velden.

Cross-model import is alleen beschikbaar als de doeladapter een geverifieerde converter heeft. De UI toont vóór toepassing welke velden exact, geconverteerd of niet ondersteund zijn. Een naamovereenkomst tussen velden is onvoldoende.

## Verificatiepoort per model

Een adapter bereikt pas volledige ondersteuning wanneer alle onderstaande stappen voor één exacte firmwareversie zijn vastgelegd:

1. officiële handleiding, firmware en PC Remote-versie geïdentificeerd;
2. credentialvrije of read-only modelidentificatie;
3. login, disconnect en bezet/fout gedrag als fixtures;
4. on-air processing- en setupdocumenten compleet geparsed;
5. presetlijst en actieve preset read-only bevestigd;
6. meterrecords, cadence, stale gedrag en kanaalbetekenis bevestigd;
7. één niet-audio of reversibele instelling geschreven, exact teruggelezen en hersteld;
8. één processingparameter in een test- of onderhoudsvenster gewijzigd en hersteld;
9. preset recall zonder sessieverlies en met doorlopende meters getest;
10. acht uur sessie- en meteringsoak zonder verkeerde reconnect of write replay;
11. capabilitygestuurde UI en onbekende-firmwaregrenzen getest;
12. documentatie, profiel en skillreferentie bijgewerkt.

Tot stap 7 blijft de adapter read-only. Onbekende firmware mag identificatie en eventueel bewezen status tonen, maar nooit automatisch schrijven.

## Minimale testopstelling

Voor de eerste implementatierondes is nodig:

- een 5500i en daarna een 5500, met firmwareversie en toegangscode die op het frontpaneel kunnen worden gecontroleerd;
- een 5700 FM of HD en een 8500 voor het afbakenen van de gedeelde `pc-remote-v2`-kern;
- een 8400 met werkende ondersteunde Ethernet-PC Card en toegang tot UDP 16540;
- een 8200, een betrouwbare USB/RS-232-adapter, een echte null-modemkabel en zo nodig een seriële breakout/analyzer;
- een geïsoleerde test- of onderhoudssituatie waarin een display-/setupwaarde en later één processingparameter veilig kunnen worden gewijzigd en hersteld;
- de exact bij de firmware horende officiële PC Remote-versie voor vergelijking en captures.

Leg bij ieder apparaat vooraf een frontpaneelbackup of bestaande officiële Remote-backup vast. Maak geen generieke writefixtures op een live uitzendprocessor zonder een afgesproken herstelpad.

## Open vragen waarvoor hardware nodig is

- 8200 seriële framing, baudratevarianten, login en meterrecordgrenzen per firmware.
- 8400 TCP-commandoframing, exacte UDP-pakketindeling en herstel na verloren datagrams.
- 8500/5500/5500i/5700/8600/8700i: exacte gelijkheid of verschillen in opcodes, archiefcrypto, sessie-heartbeat en meterrecordlengtes.
- Welke terminalbanners en model-/firmwarestrings ieder apparaat werkelijk terugstuurt.
- Of PC Remote meer dan één client toestaat en hoe `busy` of overname per model werkt.
- Welke firmwareversies binnen één profiel veilig compatibel zijn.
- Of de HTML5-generatie een ondersteunde API, WebSocketprotocol of alleen interne browserendpoints aanbiedt.

Deze vragen blokkeren niet het adapterrefactorwerk. Ze blokkeren wel claims van volledige ondersteuning en alle schrijfacties voor het betreffende model.
