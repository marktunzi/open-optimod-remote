# Open Optimod Remote — compatibiliteitsmatrix

Referentie: Optimod 5700i firmware 3.0.1.20 met bijpassende PC Remote. Stand: 29 september 2026. Deze lijst maakt het bouwplan controleerbaar; zij is nog geen afgeronde compatibiliteitscertificering.

**Status:** L = lezen geverifieerd; D = gedocumenteerd; O = nog te verifiëren. Er is inmiddels een eigen Rust-protocolkern en lokale webinterface. De native en browserproeven lezen 400 velden, ontvangen echte meters en wijzigen/herstellen schermcontrast met volledige teruglezing. Dit is nog geen complete Windows-vervanger. Zie [protocolbewijs](protocol/5700i.md) voor de actuele begrenzing. De onderstaande scenario's blijven open tenzij afzonderlijk bewezen.

Update 23 september: opgeslagen benoemde verbindingen gebruiken een Finder-achtig native AppKit-venster met `NSSplitViewController` en een echt `.sidebar` split-item; Presets gebruikt dezelfde native sidebararchitectuur. Toegangscodes staan in een afzonderlijk lokaal bestand met alleen gebruikersrechten, zonder Keychain. Network zoekt het actieve lokale IPv4-subnet af en herkent de twaalf huidige PC Remote-modellen aan hun eigen terminalbanner zonder een code te versturen. Presets openen in een tweede native venster met Factory/User/Modified-filters, on-air-status, lokale `.orb57user`-export en volledig teruggelezen bestandstoepassing. Recall gebruikt de geladen catalogus, voert één RP/AP-transactie uit terwijl PC Remote-polling gepauzeerd is, en herstart de meters daarna gecontroleerd. De laatst bevestigde opgeslagen processing is de vergelijkingsbasis; afwijkende velden worden direct cyaan en blijven dat totdat ze teruggezet zijn of een nieuwe preset/bestand de basis vormt. Interactieve bedieningen reageren direct en gebruiken host-, sessie-, preset- en oudewaardebewaking plus de geordende protocolgrens; volledige terminaldocumenten blijven voor foutafhandeling, refresh, export en presetbewerkingen. Setup gebruikt tien taakgerichte hoofdcategorieën; compacte categorieën tonen alle groepen op één pagina en grotere categorieën hebben een tweede native subtablaag. Alle 134 bedieningen worden tegen het firmwareprofiel gecontroleerd. De globale FM→HD-knop heeft de geometrie, gradients, highlights en LED uit het laatste aangeleverde ontwerp. Beide meterpaden blijven tegelijk zichtbaar; de FM/HD-editortab verschijnt alleen bij ontkoppelde processing. Kanaalspecifieke meteromzettingen, vloeiende canvasinterpolatie en een afzonderlijke 900 ms peak-hold per lane zijn actief. Absolute HD/loudness-kalibratie, apparaatgestuurde gate/overload/lock-indicaties en nog niet geverifieerde benoemde preset-savecommando's blijven open.

## Andere OPTIMOD-modellen

Update 29 september: de **5500** (firmware 1.2.8.24) en de oorspronkelijke **OPTIMOD-FM 8700HD** (firmware 1.0.2.161, banner `8700HD V`, een ander model dan de 8700i) hebben een volledig profiel met writes, recall, live meters en eigen processingpagina's. Setup plaatst elk systeemveld van de 5500, 5700i en 8700HD in dezelfde tien categorieën; het native Settings-venster bestaat alleen voor de 5700i, en System Settings… en Command-, openen voor de 5500 en 8700HD de Setup-workspace. Presets gebruikt in de app en de browser dezelfde workspace. Op elk schrijfbaar model werken presetbestanden en een backup met restore van de systeeminstellingen; alleen de 5500 kan presets op het apparaat opslaan, hernoemen en verwijderen (`SP`/`DP`). De methode reproduceert 363 van de 366 hardware-geverifieerde 5700i-mappings exact en geen enkele fout; de 5700i krijgt daarnaast 199 statisch afgeleide veldnamen (364 scope-items; two-band, MX voor units met Orbans betaalde MX-upgrade, klok, testtonen) die bij elke wijziging worden teruggelezen. Dat profiel is statisch afgeleid uit de officiële PC Remote en firmware en gecontroleerd tegen de factorypresets, maar **niet op hardware geverifieerd**. De app toont dat en leest elke wijziging volledig terug. Velden die de presets tegenspreken, zijn niet schrijfbaar. Andere firmwareversies van deze modellen blijven read-only. Zie de [statische analyse](research/2026-09-29-5500-8700hd-static-analysis.md) en [een model toevoegen](adding-a-model.md).

De 5500i, 5700 FM, 5700 HD, 6300, 8500, 8600, 8700i, 9300 en 9400, en andere firmware van de 5500, 5700i en 8700HD, kunnen nu afzonderlijk worden opgeslagen, native worden ontdekt, via hun officiële banner worden geïdentificeerd en read-only AP/AS/LP-documenten en presetlijsten leveren. Zij hebben ieder een aparte skin. Hun writes, recall en live meters zijn nog niet als ondersteund afgetekend. De 8200, 8400 en de HTML5-generatie blijven aparte onderzoeks- en implementatiedoelen. De resultaten staan in [het multi-modelonderzoek](research/2026-09-22-multi-model-optimod-support.md) en het [adapterimplementatieplan](plans/2026-09-22-multi-model-adapter-plan.md).

Belangrijkste grens: 8200 gebruikt een eigen seriële route; 8400 gebruikt TCP 51200 plus UDP 16540 voor meters; 8500/5500/5500i/5700 en meerdere latere PC Remote-modellen lijken dezelfde brede 6201-protocolfamilie te delen maar vereisen per model en firmware eigen fixtures en profielen; 5750/5950/Trio zijn HTML5 Web UI-modellen. Alleen statische overeenkomst is nooit voldoende om schrijven toe te staan.

De rij voor een functiegroep is pas klaar als ook alle onderliggende Windowsregelaars en toestanden zijn afgedekt. Een veld dat ontbreekt in de actieve multibandpreset blijft onderdeel van de inventarisatie voor andere structuren.

## Verbinding en interface

| ID | Vereiste | Huidig bewijs | Acceptatiescenario |
|---|---|---|---|
| CON-01 | Apparaten toevoegen, aliases en groepen, eigenschappen aanpassen | L/D: native macOS beheer voor meerdere benoemde verbindingen; toevoegen, wijzigen, zoeken, verwijderen en lokale netwerkdetectie werken; geen groepen | Dezelfde beheerhandelingen als Windows; configuratie blijft na herstart behouden |
| CON-02 | Ethernetverbinding met instelbare host/poort | L: TCP 23/6201 | Complete PC Remote-sessie vanaf een schone Mac zonder Windowscomponenten |
| CON-03 | Authenticatie en beperkte rechten | L: een code met alle rechten | Volledige, beperkte, verkeerde en ontbrekende code; geen ongeautoriseerde bediening |
| CON-04 | Bezet apparaat en nette disconnect | O | Frontpaneel of tweede toegestane client beëindigt sessie; UI en meters reageren correct |
| CON-05 | Firmware-/modelherkenning | L: identificatie | Onbekend model of firmware wordt expliciet behandeld; geen automatische firmwarewijziging |
| CON-06 | Wisselen tussen apparaten | D: host- en sessiecontrole | Opdrachten voor apparaat A kunnen nooit op B of een nieuwere sessie op dezelfde host belanden |
| UI-01 | Menu's, tabbladen, dialoogvensters en volgorde | L/D: definitieve instrumentbaseline; native Connections, Presets en System Settings met tien icon-tabs plus subtabs geïmplementeerd; overige Windowsdialoogbaseline onvolledig | Per overig referentiescherm positie, inhoud en acties vergelijken |
| UI-02 | Selectie, drag, muiswiel, +/− en pijltjestoetsen | D: Processing-waardevelden ondersteunen Tab, pijlen, Page Up/Down, Home/End en +/− | Eén invoerstap geeft dezelfde apparaatstap; focus en geselecteerde regelaar blijven correct |
| UI-03 | Tab-/Ctrl-Tab-navigatie en contexthelp | D | Dezelfde navigatievolgorde; eigen helptekst legt dezelfde functie uit |
| UI-04 | On-air, gewijzigd, opgeslagen, verbonden en niet verbonden | L/D: on-air preset en veldniveau-afwijkingen volgen bevestigde documenten | UI-toestanden volgen bevestigde apparaattoestand en Windowsworkflow |
| UI-05 | Schalen en venstergrootte | D: exacte 1132-basis; audit van 27 Processing/Setup-toestanden plus drie metermodi op 1440×900 en 1180×760 zonder afgekapte bedieningstekst of pagina-overflow | Geen verdwenen bediening of onbedoelde herordening; Retina en toetsenbord bruikbaar |
| UI-06 | Alle overige applicatievoorkeuren en contextmenu's | O | Windowsinventaris per menu en dialoog volledig afwerken |

## Processing en FM/HD

| ID | Vereiste | Huidig bewijs | Acceptatiescenario |
|---|---|---|---|
| PRO-01 | Basic, Full en Advanced Modify | L/D: actuele multibandvelden | Iedere regelaar heeft bewezen bereik, stappen, eenheid en schrijf-/terugleestest |
| PRO-02 | Less-More en het verliezen/behouden van die functie | L/D | Basic- en Advanced-wijzigingen en reset gedragen zich als Windows |
| PRO-03 | Stereo enhancer, phase rotator, HPF en overige gedeelde delen | L/D | Scope en gekoppelde effecten zijn identiek aan de referentie |
| PRO-04 | Alle AGC-regelaars en varianten | L/D | Attack/release/gate/window/matrix/coupling en speciale Off-waarden correct |
| PRO-05 | FM EQ: shelving en parametrische banden | L/D | Frequency/gain/width/slope en alle afhankelijke weergaven correct |
| PRO-06 | FM multiband: alle banden, mixes en koppelingen | L/D | Alle bandregelaars, attack/release/delta/threshold/gating en states correct |
| PRO-07 | Clippers, HF, bass, speech- en distortionregelaars | L/D | Alle Advanced-tabbladen en afhankelijkheden onder geschikte testomstandigheden |
| PRO-08 | Two-Band | D: velden, pagina's en meters statisch afgeleid uit 5700i PC Remote 3.0.1.20; niet actief uitgelezen op hardware | Eigen volledige velden- en meterlijst, geen overgenomen vijfbanddefaults |
| PRO-09 | Alle drie Five-Band-latencystructuren | D: MX- en ULL-velden en -pagina's statisch afgeleid; slechts actuele structuur gelezen | Juiste veldbeschikbaarheid en preset-/meterovergangen |
| HD-01 | FM/HD-weergave wisselen | L: HD-velden, D: gedrag | Geen enkele audio-instelling verandert uitsluitend door een andere weergave te kiezen |
| HD-02 | FM→HD-koppeling en Independent | L/D: gelezen; officiële 3.0-handleiding en originele dialooglayout bevestigen dat alleen tegenhanger-regelaars volgen; regressietests houden HD Limiting actief | FM-wijziging volgt alleen volgens de juiste koppeling; Independent bewaart HD-waarden |
| HD-03 | HD EQ en multiband | L/D | Zelfstandige HD-bediening en alle koppelingen zoals PC Remote |
| HD-04 | HD limiter, De-Ess, HF shelf en overige HD processing | L/D: unieke HD Limiting-regelaars (`IBOC EQ GAIN/FREQ`, `IBOC LIM DR`, `HD DE ESS`) blijven bedienbaar bij FM→HD | Iedere HD-regelaar correct, inclusief structuurspecifieke beschikbaarheid |
| HD-05 | Bandbreedte, mono/stereo, polariteit en loudness | L/D | Zelfde opties, eenheden en invloed op de juiste HD-scope |
| HD-06 | FM-only apparaat en optionele HD-upgrade | D | Onbeschikbare functies correct markeren; geen hardwareopties omzeilen |

## Live meters

| ID | Vereiste | Huidig bewijs | Acceptatiescenario |
|---|---|---|---|
| MET-01 | Alle input- en AGC-meters | L/D: live kanaaldata en referentiecurves | Kanaal, schaal, GR-keuze en actualisatie vergelijken met Windows |
| MET-02 | Alle FM-processing- en uitgangsmeters | L/D: live kanaaldata en display-derived peak hold; device peak/clip O | Zelfde kanalen voor iedere structuur, inclusief indicaties en clip/peakgedrag |
| MET-03 | Alle HD-processing- en uitgangsmeters | L/D: live kanaaldata; absolute assen O | Echte HD-data; FM/HD-weergave geeft nooit verkeerd gelabelde frames |
| MET-04 | MPX- en loudnessweergaven waar PC Remote ze aanbiedt | L/D: live data; absolute assen O | Juiste eenheden, reset-/holdgedrag en referentiewaarden |
| MET-05 | Overload, gating, signal locks en overige indicatoren | L: enkele statusvelden; volledige lijst O | Alle referentie-indicatoren afwerken, inclusief onbekend/geen data |
| MET-06 | Latentie en meterballistiek | L/D: gemiddeld 55,3 ms pakketritme; clientinterpolatie en onafhankelijke 900 ms peaks getest | Meetfrequentie, peak-timing en beweging vergelijken met Windows; geen zichtbare sprongen tussen geldige pakketten |
| MET-07 | Uitval en duurtest | O | Geen fictieve of als live getoonde bevroren data; acht uur stabiel zonder oplopende buffers |

## Systeem, bestanden en onderhoud

| ID | Vereiste | Huidig bewijs | Acceptatiescenario |
|---|---|---|---|
| SYS-01 | Analoog/digitaal input, referentieniveaus, balans en fallback | L/D | Alle instellingen en speciale modes; gecontroleerde teruglezing |
| SYS-02 | Analoog, AES1, AES2: levels, routing, format, dither, sync | L/D | Elke Windowsoptie; schermkeuze FM/HD blijft gescheiden van outputrouting |
| SYS-03 | Stereo-encoder, composite, pilot, SSB/mono en MPX | L/D | Alle parameters en afhankelijke bediening, getest buiten ongecontroleerde uitzending |
| SYS-04 | Diversity delay, enable per uitgang, trim en polariteit | L/D | Exacte getalsrepresentatie en stapgrootte; geen afrondingsdrift |
| SYS-05 | Algemene processinginstellingen, pre-emphasis en loudness | L/D | Alle systeemopties en hun gevolgen voor de processingweergave |
| SYS-06 | Bypass, testtonen en lijninstelling | L/D | Identieke modes en terugkeer naar vorige toestand, op testopstelling |
| SYS-07 | Netwerk, poorten, tijd en synchronisatie | L/D | Alle PC Remote-instellingen; verwachte disconnect na netwerkverandering wordt afgehandeld |
| SYS-08 | GPI, tallies en silence-detect | L/D | Volledige functie-toewijzingen, status en timing |
| SYS-09 | Passcodes, rechten en frontpanel lockout waar PC Remote beschikbaar | D: toegangsniveau als instelling zichtbaar; passcodes maken, wijzigen of verwijderen niet geïmplementeerd | Rechten niet alleen in UI afdwingen; herstellen van toegang blijft mogelijk |
| SYS-10 | SNMP-configuratie | L/D | Alle manager-, community- en enable-instellingen volgens referentie |
| RDS-01 | PS/RT/PI/PTY/PTYN, MS/DI/TP/TA, AF en timing | L/D; schrijftests O | Alle systeem- en presetgebonden RDS-functies |
| RDS-02 | Subcarrier, terminalinstellingen, UECP en EAS-functies | L/D | Bereik/format/bronbeperkingen en prioriteiten volgen apparaatgedrag |
| OPT-01 | Ratings/Kantar/Nielsen en overige aanwezige opties | L: enkele velden; hardwaretests O | Alleen beschikbare functies; aanvullende hardware nodig voor definitieve validatie |
| PRE-01 | Presetlijst en on-air-identiteit | L: 75 presets op referentieapparaat; eigen native venster; saved/modified vergelijking per veld | Factory/user/modified correct; geen onbedoelde recall bij selecteren |
| PRE-02 | Recall en terugwisselen huidige/vorige preset | D | Dubbelklik/Recall en herhaald Recall exact vergelijken |
| PRE-03 | Opslaan, Save As en overige presetbeheeracties | D/O: lokaal actueel document opslaan werkt op elk schrijfbaar model; 5500: opslaan, hernoemen en verwijderen op het apparaat met `SP`/`DP` en controle via de presetlijst (synthetisch getest, niet op hardware); 5700i en 8700HD: geen terminalopdracht, blijft open | Alle acties die de referentie aanbiedt; expliciet gedrag bij naamconflict en unsaved preset |
| PRE-04 | Lokale en apparaatbestanden synchroniseren | D: lokaal bestand toepassen valideert en bevestigt ieder veld en het volledige einddocument | Correcte richting en bestaande bestanden behouden; apparaat blijft autoritatief |
| PRE-05 | Volledige backup en restore | D/O: backup van systeemdocument, on-air-preset en presetnamen; restore van systeeminstellingen met plan vooraf en teruglezing (netwerk en klok uitgezonderd). Userpresets die niet on-air staan en automation blijven open | Userpresets, setup en automation; herstelde waarden en gedrag controleren |
| PRE-06 | Ondersteunde Orban-bestandsimport/export en encryptie | D: lokale AP-documentexport (`.orb55user`, `.orb57user`, `.orb86user`) en veldgewijze toepassing geïmplementeerd; versleutelde Orban-archieven en overige formaten O | Versiematched fixturelijst; juiste codes vragen; onbekende velden behouden waar ondersteund |
| PRE-07 | Legacy-presetimport die PC Remote ondersteunt | D; omzetting O | Alle daadwerkelijk aangeboden formaten; niet slechts succesvolle bestandsselectie |
| AUT-01 | Automation maken, wijzigen, verwijderen en aan/uit | D | Events worden op het apparaat opgeslagen en uitgevoerd; app hoeft niet open te blijven |
| AUT-02 | Tijd/datum, events en gekoppelde acties | D | Zelfde tijdinterpretatie, week-/dagopties en andere referentieacties |
| MNT-01 | Diagnostiek, logs, informatie en servicefuncties uit PC Remote | L: deels status | Volledige menu-inventaris; bestaande hardwaremeldingen apart van appfouten |
| MNT-02 | Software-/firmware-updatefunctie van PC Remote | D; implementatie O | Gevalideerde bestands-/versiecontroles en onderbrekingsgedrag op geschikte testhardware |
| MNT-03 | Alle overige tools-, help- en onderhoudsacties | O | Geen openstaande referentiemenu's bij volledige release |

## Robuustheid, distributie en scope

| ID | Vereiste | Huidig bewijs | Acceptatiescenario |
|---|---|---|---|
| REL-01 | Reconnect en slaapstand | O | Geen automatische replay van oude wijzigingen; complete resynchronisatie |
| REL-02 | Externe veranderingen en concurrency | O | Frontpaneelwijzigingen zichtbaar; geen vals atomair schrijfgedrag beloven |
| REL-03 | Fouten bij preset-/bestandsoverdracht | O | Gedeeltelijke overdracht geeft geen succes; herstelpad getest |
| REL-04 | Schone Mac-installatie | O | Alleen eigen release nodig; geen Orban-executable, Wine, CrossOver of Windowsdienst |
| OSS-01 | Bouwbare openbare broncode en licentie | Plan: MIT voor eigen code | Build, tests, bijdragen en firmwarecompatibiliteit gedocumenteerd |
| OSS-02 | Geschoonde fixtures en herkomst | O | Geen codes, persoonlijke presets of gesloten software in repository/release |
| WEB-01 | Dezelfde UI via lokale server | L/D: loopback-webinterface is de huidige uitvoering | Dezelfde scenario's via browser; authenticatie en correcte verbindingslevensduur |
| TRN-01 | Serial/PPP/modem | Expliciete scopebeslissing M0 | Als uitgesteld: zichtbaar vermelden dat release Ethernet ondersteunt; geen volledige transportpariteit claimen |

## Aftekenen

Een testbewijs bevat model, firmware, app-build, referentieversie, scenario, invoer, apparaatresultaat en datum. Waar een numerieke of gedragsafwijking optreedt, blijft de rij open. “Werkt vermoedelijk” is geen acceptatiestatus.

Deze matrix moet in M0 worden aangevuld uit de complete Windowsreferentie. De huidige lijst dekt de bekende functiegroepen, maar vervangt die noodzakelijke inventarisatie niet.
