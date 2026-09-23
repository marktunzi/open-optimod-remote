# Implementatieplan: multi-model OPTIMOD-adapters

> **Status:** In uitvoering. Fase 0–2, de read-only basis van fase 3–4 en de read-only PC Remote-modellen uit fase 8 zijn op 23 september 2026 geïmplementeerd.

Datum: 22 september 2026
Brononderzoek: [multi-model support](../research/2026-09-22-multi-model-optimod-support.md)

## Doel

Breid Open Optimod Remote uit van één hardgecodeerde 5700i naar een capabilitygestuurde app waarin meerdere OPTIMOD-modellen kunnen worden opgeslagen, veilig worden gedetecteerd en via een geverifieerde model-/firmwareadapter worden bediend. De bestaande 5700i mag tijdens de refactor geen functionele regressie krijgen.

Ieder model heeft verplicht een eigen `skin_id`, productmarkering, materiaal- en kleurprofiel, displaystijl, metergroepering en set zichtbare pagina's. Een model mag nooit visueel of functioneel terugvallen op de 5700i-skin.

## Geïmplementeerd op 23 september 2026

- Adapterregister met afzonderlijke IDs voor 5700i, 5500i, 5500, 5700 FM, 5700 HD, 6300, 8500, 8600, 8700i, 9300 en 9400.
- Herkenning van de officiële PC Remote-loginprefixen `5700i V`, `5500i V`, `5500 V`, `5700FM V`, `5700HD V`, `6300 V`, `8500 V`, `8600 V`, `8700i V`, `9300 V` en `9400 V`.
- Fail-closed controle tussen opgeslagen modelkeuze en werkelijk gedetecteerde loginbanner.
- Read-only AP/AS/LP-terminalondersteuning voor de `8300`, `5700`, `6300`, `8500`, `8600`, `8700`, `9300` en `9400`-documentfamilies die door officiële handleidingen, executables en factorypresets zijn onderbouwd.
- Verbindingsschema versie 2 met automatische migratie van bestaande apparaten naar Auto Detect, zonder wijziging van credentialaccounts.
- Native macOS-modelkeuze en netwerkdetectie voor de elf Ethernetmodellen.
- Afzonderlijke skins en originele lokale productmarkeringen voor ieder model, plus een neutrale skin voor onbekende/offline toestand.
- Per-model capabilitygrenzen: alleen 5700i firmware 3.0.1.20 heeft writes, recall en de 112-byte live-metervoorstelling ingeschakeld.

Nog niet afgetekend zijn model-specifieke writeprofielen, metercaptures en volledige bedieningspagina's voor de tien aanvullende modellen. Zij blijven read-only totdat de hardwarepoort per exact model en firmware is doorlopen.

## Fase 0 — referentiegedrag vastzetten

**Status: gereed voor de huidige fixtures.**

- Voeg fixtures toe voor 5700i-login, framing, documentdialect, twee meterbanken, presetlijst, recall en disconnect.
- Leg de huidige 5700i-capabilities vast in een machineleesbaar bestand.
- Test dat onbekend model en onbekende firmware fail-closed blijven.
- Test dat de huidige write-, recall- en metercontinuïteit ongewijzigd blijft.

**Klaar wanneer:** alle bestaande tests plus adaptercontracttests dezelfde 5700i-uitvoer geven.

## Fase 1 — adapterregister zonder gedragswijziging

**Status: adapter- en capabilityregister gereed; verdere opsplitsing van profilebestanden volgt per hardwaretarget.**

- Introduceer `ModelId`, `FirmwareId`, `AdapterId`, `TransportKind` en `CapabilitySet`.
- Verplaats de 5700i-controles uit `session.rs`, `terminal.rs`, `document.rs`, `message.rs` en `profile.rs` naar `adapters/optimod_5700i`.
- Houd framing, archivecrypto en parsers als deelbare modules; de adapter bepaalt de grenzen.
- Maak parameter-, meter- en UI-profielen runtime-selecteerbaar.
- Laat onbekende adapters alleen een veilige foutstatus opleveren.

**Klaar wanneer:** de app nog steeds uitsluitend 5700i 3.0.1.20 ondersteunt, maar nergens meer via globale modelconstanten hoeft te beslissen.

## Fase 2 — verbindingenschema versie 2

**Status: gereed voor de huidige Ethernetmodellen. Seriële velden volgen in fase 6.**

- Voeg modelmodus, gedetecteerd model/firmware, transportsoort en adapter-ID toe.
- Migreer versie 1 atomair en behoud alle bestaande opgeslagen apparaten en toegangscodes.
- Maak het native verbindingenvenster modelbewust: Auto Detect en expliciete modelkeuze voor legacy/serieel.
- Toon geverifieerde model- en firmware-informatie in de verbindingenlijst.
- Gebruik per adapter passende velden: IP en poorten voor Ethernet; poort, baudrate en flow control voor serieel.

**Klaar wanneer:** een bestaande gebruiker zonder handwerk migreert en een foutieve modelkeuze nooit een write activeert.

## Fase 3 — 5500i read-only en daarna volledig

**Status: read-only identificatie, AP/AS-documenten, presetlijst, native selectie en eigen skin gereed. Writes en live meters wachten op hardwarefixtures.**

- Verzamel een 5500i met exacte firmware en bijpassende officiële Remote-versie.
- Capture login, heartbeat, documenten, presetlijst, meters en disconnect.
- Bouw `pc-remote-v2` als gedeelde sessiekern met een 5500i-adapter.
- Maak 5500i parameter-, meter- en capabilityprofielen.
- Bouw FM/stereo-encoder/diversity-UI zonder 5700i HD-tabs.
- Verifieer één reversibele systeemwrite, daarna processingwrite en recall.

**Klaar wanneer:** de volledige modelverificatiepoort uit het onderzoeksdocument is doorlopen.

## Fase 4 — 5500, 5700 FM/HD en 8500

**Status: dezelfde read-only basis als 5500i is gereed; eigen parameter-, meter- en writeprofielen volgen.**

- Voeg per model een afzonderlijk profiel en firmwarematcher toe.
- Deel alleen framing, login, archive en terminalcode die door fixtures identiek blijkt.
- Modelleer 5700 FM en 5700 HD als verschillende capabilities.
- Modelleer 8500 FM/HD, coupling en meters los van de 5700i-layout.
- Voeg geverifieerde presetconverters toe voor officieel ondersteunde richtingen.

**Klaar wanneer:** ieder model zelfstandig kan worden uitgeschakeld zonder een gedeelde protocolmodule te breken.

## Fase 5 — 8400 TCP plus UDP

- Bouw `legacy-8400-tcp-udp` met een TCP-owner en een afzonderlijke UDP-meterreceiver.
- Voeg timeout, sequence/loss-detectie en stale meterstatus toe zonder besturingssessie te verbreken.
- Ondersteun het `00.08`-documentdialect en eigen veldprofiel.
- Voeg Network-detectie toe op bewezen, credentialvrije identificatie.
- Test ook de toestand waarin TCP werkt maar UDP door de firewall wordt geblokkeerd.

**Klaar wanneer:** besturing en meters onafhankelijk herstellen en geen 5700i-aanname actief is.

## Fase 6 — 8200 serieel

- Voeg een seriële transportlaag met exclusieve apparaatlock toe.
- Bouw een 8200-specifieke login-, framing-, meter- en documentadapter vanuit eigen hardwarecaptures.
- Voeg seriële apparaten toe aan de native connection editor.
- Ondersteun directe USB/RS-232 en optioneel een door de gebruiker beheerde serial-to-IP-brug.
- Test kabelverlies, poortbezetting, reconnect en gedeeltelijke records.

**Klaar wanneer:** een 8200 zonder Windows Remote volledig kan worden gelezen en de geverifieerde functies veilig kan uitvoeren.

## Fase 7 — meerdere gelijktijdige sessies

- Vervang de ene `Option<Device>` door een runtime per `DeviceId`.
- Geef iedere runtime eigen status, meters, caches, queues en foutgeschiedenis.
- Maak de geselecteerde processor expliciet in alle mutatieverzoeken.
- Voer een instelbare limiet voor gelijktijdige verbindingen in.
- Voeg een overzichtsmodus toe met compacte meters en alarmstatus per processor.

**Klaar wanneer:** een schrijfopdracht voor apparaat A onder geen enkele wissel-, reconnect- of recallrace naar apparaat B kan gaan.

## Fase 8 — overige PC Remote- en Web UI-modellen

- **Status:** 6300, 8600, 8700i, 9300 en 9400 hebben read-only identificatie, AP/AS/LP-documenten, presetlijsten, native selectie en ieder een eigen skin. Writes en live meters wachten op hardwarefixtures.
- Doorloop voor 6300, 8600, 8700i, 9300 en 9400 dezelfde resterende verificatiepoort voor writes, meters en volledige bedieningspagina's.
- Bouw aparte AM- en TV/digital-audiocapabilities.
- Onderzoek 5750/5950/Trio via officiële API-documentatie of geschoonde captures van eigen hardware.
- Implementeer de HTML5-generatie in een afzonderlijke adapter; hergebruik geen 6201-aannames.

## Teststrategie

- Contracttests voor ieder adapteronderdeel.
- Opgenomen, geschoonde fragmented/coalesced socketfixtures.
- Eigenschapstests voor framing- en documentgrenzen.
- Migratietests voor verbindingenboek versie 1 naar 2.
- UI-tests waarin capabilities tabs, meters en velden toevoegen of verwijderen.
- Hardwaretests per exact model/firmware, met een reversibele write en volledige restore.
- Recalltests die verbinding en meterstream bewaken.
- Acht uur soak per adapter voor een releaseclaim.

## Niet combineren

- Voeg gelijktijdige multi-devicebediening niet toe tijdens de eerste adapterrefactor.
- Combineer geen veldprofielen op basis van gelijke veldnamen.
- Behandel presetimport niet als bewijs voor protocolcompatibiliteit.
- Voeg geen fallback toe die bij een onbekend model het 5700i-profiel probeert.
- Publiceer geen model als ondersteund voordat de hardwareverificatiepoort is afgerond.
