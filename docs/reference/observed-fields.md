# Open Optimod Remote — uitgelezen veldinventaris

14 september 2026. Optimod 5700i firmware 3.0.1.20. Deze bijlage bevat uitsluitend veldnamen en apparaattypen; geen toegangscodes of persoonlijke instelwaarden.

**Status van alle onderstaande velden: rechtstreeks uitgelezen; schrijfprotocol, volledige waardetabel en Windowsbediening nog niet geverifieerd.**

Dit zijn velden van de huidige multibandpreset en systeemconfiguratie. Zij zijn niet automatisch 400 zichtbare of schrijfbare Windowsregelaars. Andere algoritmen, rechten, meters, netwerk-/automationbestanden en hardwareopties vereisen aanvullende inventarisatie.

`Cent`, `Int`, `String` en `UserString` zijn de ontvangen typeaanduidingen. Omrekening, eenheid, enumvolgorde en de betekenis van de aparte `D`-index moeten per veld worden bewezen; een index is niet zomaar een fysieke waarde.

## Processing — actieve multibandpreset

222 unieke ontvangen veldnamen.

| ID | Apparaatveld | Ontvangen type |
|---|---|---|
| P-001 | HD COUPLING | String |
| P-002 | AGC | String |
| P-003 | AGC DRIVE | Int |
| P-004 | IDLE GR | Cent |
| P-005 | GATE THRESH | Int |
| P-006 | AGC RELEASE | Cent |
| P-007 | AGC MAST DELTA TH | Cent |
| P-008 | AGC BASS TH | Cent |
| P-009 | AGC BASS DELTA TH | Cent |
| P-010 | AGC BASS ATTACK | Cent |
| P-011 | AGC BASS RELEASE | Cent |
| P-012 | AGC MASTER ATTACK | Cent |
| P-013 | AGC DIFF GR | String |
| P-014 | AGC WINDOW THRESH | Cent |
| P-015 | AGC WINDOW RELEASE | Cent |
| P-016 | AGC XOVER | String |
| P-017 | AGC MATRIX | String |
| P-018 | AGC RATIO | String |
| P-019 | AGC BASS COUPLE | Int |
| P-020 | MB GATE THR | Int |
| P-021 | BASS CLIP | Cent |
| P-022 | SPEECH BC THR | Cent |
| P-023 | BASS CLIP SHAPE | Cent |
| P-024 | MB DRIVE | Int |
| P-025 | MB RELEASE | String |
| P-026 | B12 CROSSOVER | String |
| P-027 | B1 DELTA REL | Int |
| P-028 | B2 DELTA REL | Int |
| P-029 | B3 DELTA REL | Int |
| P-030 | B4 DELTA REL | Int |
| P-031 | B5 DELTA REL | Int |
| P-032 | B1 LIMIT ATTACK | Int |
| P-033 | B2 LIMIT ATTACK | Int |
| P-034 | B3 LIMIT ATTACK | Int |
| P-035 | B4 LIMIT ATTACK | Int |
| P-036 | B5 LIMIT ATTACK | Int |
| P-037 | B1 OUTPUT MIX | Cent |
| P-038 | B2 OUTPUT MIX | Cent |
| P-039 | B3 OUTPUT MIX | Cent |
| P-040 | B4 OUTPUT MIX | Cent |
| P-041 | B5 OUTPUT MIX | Cent |
| P-042 | BASS COUPLING | Int |
| P-043 | BAND 23 COUPL | Int |
| P-044 | BAND 32 COUPL | Int |
| P-045 | BAND 34 COUPL | Int |
| P-046 | BAND 45 COUPL | Int |
| P-047 | B1 COMP THRSH | Cent |
| P-048 | B2 COMP THRSH | Cent |
| P-049 | B3 COMP THRSH | Cent |
| P-050 | B4 COMP THRSH | Cent |
| P-051 | B5 COMP THRSH | Cent |
| P-052 | B1 ATTACK | Int |
| P-053 | B2 ATTACK | Int |
| P-054 | B3 ATTACK | Int |
| P-055 | B4 ATTACK | Int |
| P-056 | B5 ATTACK | Int |
| P-057 | B1 DIFF GR | String |
| P-058 | B2 DIFF GR | String |
| P-059 | B3 DIFF GR | String |
| P-060 | B4 DIFF GR | String |
| P-061 | B5 DIFF GR | String |
| P-062 | B1 ON/OFF | String |
| P-063 | B2 ON/OFF | String |
| P-064 | B3 ON/OFF | String |
| P-065 | B4 ON/OFF | String |
| P-066 | B5 ON/OFF | String |
| P-067 | DWNWRD EXP | String |
| P-068 | B5 DWNWRD EXP | Cent |
| P-069 | FINAL CLIP DRV | Cent |
| P-070 | FC HEADROOM | Cent |
| P-071 | BRILLIANCE | Cent |
| P-072 | PHASE ROTATOR | String |
| P-073 | MULTIBAND CLIP | Cent |
| P-074 | B3 CLIP THRSH | Cent |
| P-075 | MAX DIST CONTRL | Cent |
| P-076 | SPEECH THR | Cent |
| P-077 | B5 CLIP THRSH | Cent |
| P-078 | HF LIMITER | Cent |
| P-079 | CLIP W2 | Cent |
| P-080 | DRIVE W2 | Cent |
| P-081 | BASS CLIP MODE | String |
| P-082 | COMP LOOKAHEAD SW | String |
| P-083 | HF CLIPPING | Cent |
| P-084 | SP B1 ATTACK | Int |
| P-085 | SP B2 ATTACK | Int |
| P-086 | SP B3 ATTACK | Int |
| P-087 | SP B4 ATTACK | Int |
| P-088 | SP B5 ATTACK | Int |
| P-089 | SP B1 COMP THRSH | Cent |
| P-090 | SP B2 COMP THRSH | Cent |
| P-091 | SP B3 COMP THRSH | Cent |
| P-092 | SP B4 COMP THRSH | Cent |
| P-093 | SP B5 COMP THRSH | Cent |
| P-094 | SP MB RELEASE | String |
| P-095 | SP MAX DIST CONTRL | Cent |
| P-096 | 30 HZ HPF | String |
| P-097 | DJ BASS BOOST | String |
| P-098 | HF ENHANCER | Int |
| P-099 | LOW BASS GAIN | Int |
| P-100 | LOW BASS FREQ | Int |
| P-101 | LOW BASS Q | Int |
| P-102 | PEQ LOW GAIN | Cent |
| P-103 | PEQ LOW FREQ | Cent |
| P-104 | PEQ LOW WIDTH | Cent |
| P-105 | PEQ MID GAIN | Cent |
| P-106 | PEQ MID FREQ | Int |
| P-107 | PEQ MID WIDTH | Cent |
| P-108 | PEQ HIGH GAIN | Cent |
| P-109 | PEQ HIGH FREQ | Cent |
| P-110 | PEQ HIGH WIDTH | Cent |
| P-111 | DE STEREO COUPL | String |
| P-112 | SE AMOUNT | Cent |
| P-113 | SE IN OUT | String |
| P-114 | SE RATIO WIDTH | Int |
| P-115 | SE DIFFUSION | Cent |
| P-116 | SE STYLE | String |
| P-117 | SE DEPTH | Int |
| P-118 | PILOT PROTECT | String |
| P-119 | COMP DRIVE | Cent |
| P-120 | SP DET OVERRIDE | String |
| P-121 | IBOC LIM DR | Cent |
| P-122 | HD BASS CLIP | String |
| P-123 | HD SPEECH BC THR | String |
| P-124 | HD BASS CLIP SHAPE | Cent |
| P-125 | HD DE ESS | Cent |
| P-126 | IBOC EQ FREQ | Cent |
| P-127 | IBOC EQ GAIN | Cent |
| P-128 | HD LOW BASS GAIN | Int |
| P-129 | HD LOW BASS FREQ | Int |
| P-130 | HD LOW BASS Q | Int |
| P-131 | HD PEQ LOW GAIN | Cent |
| P-132 | HD PEQ LOW FREQ | Cent |
| P-133 | HD PEQ LOW WIDTH | Cent |
| P-134 | HD PEQ MID GAIN | Cent |
| P-135 | HD PEQ MID FREQ | Int |
| P-136 | HD PEQ MID WIDTH | Cent |
| P-137 | HD PEQ HIGH GAIN | Cent |
| P-138 | HD PEQ HIGH FREQ | Cent |
| P-139 | HD PEQ HIGH WIDTH | Cent |
| P-140 | HD BRILLIANCE | Cent |
| P-141 | HD HF ENHANCER | Int |
| P-142 | HD DJ BASS BOOST | String |
| P-143 | HD B1 OUTPUT MIX | Cent |
| P-144 | HD B2 OUTPUT MIX | Cent |
| P-145 | HD B3 OUTPUT MIX | Cent |
| P-146 | HD B4 OUTPUT MIX | Cent |
| P-147 | HD B5 OUTPUT MIX | Cent |
| P-148 | HD B1 ON/OFF | String |
| P-149 | HD B2 ON/OFF | String |
| P-150 | HD B3 ON/OFF | String |
| P-151 | HD B4 ON/OFF | String |
| P-152 | HD B5 ON/OFF | String |
| P-153 | HD MB DRIVE | Int |
| P-154 | HD MB RELEASE | String |
| P-155 | HD DWNWRD EXP | String |
| P-156 | HD B5 DWNWRD EXP | Cent |
| P-157 | HD DE STEREO COUPL | String |
| P-158 | HD MB GATE THR | Int |
| P-159 | HD BAND 21 COUPL | Int |
| P-160 | HD BAND 23 COUPL | Int |
| P-161 | HD BAND 32 COUPL | Int |
| P-162 | HD BAND 34 COUPL | Int |
| P-163 | HD BAND 45 COUPL | Int |
| P-164 | HD MULTIBAND CLIP | Cent |
| P-165 | HD B1 COMP THRSH | Cent |
| P-166 | HD B2 COMP THRSH | Cent |
| P-167 | HD B3 COMP THRSH | Cent |
| P-168 | HD B4 COMP THRSH | Cent |
| P-169 | HD B5 COMP THRSH | Cent |
| P-170 | HD B1 ATTACK | Int |
| P-171 | HD B2 ATTACK | Int |
| P-172 | HD B3 ATTACK | Int |
| P-173 | HD B4 ATTACK | Int |
| P-174 | HD B5 ATTACK | Int |
| P-175 | HD B1 LIMIT ATTACK | Int |
| P-176 | HD B2 LIMIT ATTACK | Int |
| P-177 | HD B3 LIMIT ATTACK | Int |
| P-178 | HD B4 LIMIT ATTACK | Int |
| P-179 | HD B5 LIMIT ATTACK | Int |
| P-180 | HD B1 DELTA REL | Int |
| P-181 | HD B2 DELTA REL | Int |
| P-182 | HD B3 DELTA REL | Int |
| P-183 | HD B4 DELTA REL | Int |
| P-184 | HD B5 DELTA REL | Int |
| P-185 | HD B1 DIFF GR | Int |
| P-186 | HD B2 DIFF GR | Int |
| P-187 | HD B3 DIFF GR | Int |
| P-188 | HD B4 DIFF GR | Int |
| P-189 | HD B5 DIFF GR | Int |
| P-190 | HD B12 CROSSOVER | String |
| P-191 | HD COMP LOOKAHEAD SW | String |
| P-192 | HD SP MB RELEASE | String |
| P-193 | HD SP B1 COMP THRSH | Cent |
| P-194 | HD SP B2 COMP THRSH | Cent |
| P-195 | HD SP B3 COMP THRSH | Cent |
| P-196 | HD SP B4 COMP THRSH | Cent |
| P-197 | HD SP B5 COMP THRSH | Cent |
| P-198 | HD SP B1 ATTACK | Int |
| P-199 | HD SP B2 ATTACK | Int |
| P-200 | HD SP B3 ATTACK | Int |
| P-201 | HD SP B4 ATTACK | Int |
| P-202 | HD SP B5 ATTACK | Int |
| P-203 | COMPOSITE MODE | String |
| P-204 | MPX PWR OFFSET | Cent |
| P-205 | MPX PWR GATE CTRL | Int |
| P-206 | RDS USE SYSTEM | String |
| P-207 | RDS DYNAMIC PS | UserString |
| P-208 | RDS DPS SPEED | Int |
| P-209 | RDS DPS TIMEOUT | String |
| P-210 | RDS RADIO TEXT | UserString |
| P-211 | RDS RADIO TEXT SPEED | String |
| P-212 | RDS PROGRAM ID | UserString |
| P-213 | RDS PROGRAM TYPE | UserString |
| P-214 | RDS PROGRAM NAME | UserString |
| P-215 | RDS MUSIC SPEECH | String |
| P-216 | RDS DECODER INFO | String |
| P-217 | RDS TRAFFIC PROGRAM | String |
| P-218 | RDS ACTIVE | String |
| P-219 | RDS LEVEL | Cent |
| P-220 | RDS EAS | UserString |
| P-221 | RDS EAS TIME | Int |
| P-222 | LESS MORE | Cent |

## Systeem — actieve configuratie

178 unieke ontvangen veldnamen.

| ID | Apparaatveld | Ontvangen type |
|---|---|---|
| S-001 | ALGORITHM | String |
| S-002 | INPUT A OR D | String |
| S-003 | ANALOG FALLBACK | String |
| S-004 | DIGITAL FALLBACK | String |
| S-005 | DI ANALOG FALLBACK | String |
| S-006 | SILENCE THR | Int |
| S-007 | SILENCE DELAY | Int |
| S-008 | ACTUAL A OR D | String |
| S-009 | DI REF LEVEL | Cent |
| S-010 | DI REF PPM LEVEL | Cent |
| S-011 | AI REF LEVEL | Cent |
| S-012 | AI REF PPM LEVEL | Cent |
| S-013 | AI CLIP LEVEL | Cent |
| S-014 | AI BALANCE | Cent |
| S-015 | DI BALANCE | Cent |
| S-016 | AO1 LEVEL | Cent |
| S-017 | FM POLARITY | String |
| S-018 | DO1 LEVEL | Cent |
| S-019 | DO1 PRE EMPH | String |
| S-020 | DO1 WORD_LENGTH | String |
| S-021 | DO1 DITHER | String |
| S-022 | DO1 SAMPLE RATE | String |
| S-023 | DO1 SYNC | String |
| S-024 | DO1 FORMAT | String |
| S-025 | AO1 SOURCE | String |
| S-026 | DO1 SOURCE | String |
| S-027 | DO2 SOURCE | String |
| S-028 | PHONES OUT SOURCE | String |
| S-029 | DO2 LEVEL | Cent |
| S-030 | DO2 SAMPLE RATE | String |
| S-031 | DO2 WORD_LENGTH | String |
| S-032 | DO2 SYNC | String |
| S-033 | DO2 DITHER | String |
| S-034 | DO2 FORMAT | String |
| S-035 | DO2 PRE EMPH | String |
| S-036 | PILOT SYNC | String |
| S-037 | DIVERSITY DELAY ADJ | String |
| S-038 | DIVERSITY DELAY | String |
| S-039 | PILOT LEVEL | Cent |
| S-040 | PILOT REF | String |
| S-041 | ITU412 THR | String |
| S-042 | CLIP DEFEAT | String |
| S-043 | SHOW IP | String |
| S-044 | COMP1 OUT | Cent |
| S-045 | COMP2 OUT | Cent |
| S-046 | PILOT | String |
| S-047 | MODULATION | String |
| S-048 | BS1770 LDNES CTRL THR | Cent |
| S-049 | BS1770 SAFETY LIMITER | String |
| S-050 | FM BS1770 LDNES CTRL THR | Cent |
| S-051 | FM BS1770 SAFETY LIMITER | String |
| S-052 | LDNES METER UNITS | String |
| S-053 | STUDIO CHASSIS | String |
| S-054 | MOD REDUCE 1 | Cent |
| S-055 | MOD REDUCE 2 | Cent |
| S-056 | MOD REDUC SW1 | Int |
| S-057 | MOD REDUC SW2 | Int |
| S-058 | PRE-E | String |
| S-059 | AO PRE-OUT | String |
| S-060 | BYPASS GAIN | Cent |
| S-061 | FREQUENCY | Cent |
| S-062 | MOD LEVEL | Cent |
| S-063 | MOD TYPE | String |
| S-064 | TEST PILOT | String |
| S-065 | Monitor Mute | String |
| S-066 | STATION ID | UserString |
| S-067 | DAYLIGHT MONTH | String |
| S-068 | DAYLIGHT BEGINS | String |
| S-069 | STANDARD MONTH | String |
| S-070 | DAYLIGHT ENDS | String |
| S-071 | SECURITY TIMEOUT | String |
| S-072 | VIEW METERS | String |
| S-073 | SECURITY ACTIVE | Int |
| S-074 | CONTRAST | Int |
| S-075 | AUTOMATION ENABLED | String |
| S-076 | INTERFACE TYPE | String |
| S-077 | NETWORK IP ADDRESS | UserString |
| S-078 | NETWORK SUBNET MASK | UserString |
| S-079 | NETWORK GATEWAY | UserString |
| S-080 | NETWORK PORT | Int |
| S-081 | TERMINAL PORT | Int |
| S-082 | MODEM INIT STRING | UserString |
| S-083 | PRIM SNMP ADDRESS | UserString |
| S-084 | PRIM SNMP PORT | Int |
| S-085 | SECOND SNMP ADDRESS | UserString |
| S-086 | SECOND SNMP PORT | Int |
| S-087 | SNMP READ | UserString |
| S-088 | SNMP WRITE | UserString |
| S-089 | START SNMP | String |
| S-090 | REMOTE CONTACT 1 | UserString |
| S-091 | REMOTE CONTACT 2 | UserString |
| S-092 | REMOTE CONTACT 3 | UserString |
| S-093 | REMOTE CONTACT 4 | UserString |
| S-094 | REMOTE CONTACT 5 | UserString |
| S-095 | REMOTE CONTACT 6 | UserString |
| S-096 | REMOTE CONTACT 7 | UserString |
| S-097 | REMOTE CONTACT 8 | UserString |
| S-098 | TALLY 1 | String |
| S-099 | TALLY 2 | String |
| S-100 | IBOC METER | String |
| S-101 | HD EQ LOCATE | String |
| S-102 | HD BW | Cent |
| S-103 | HD STEREO MONO | String |
| S-104 | HD POLARITY | String |
| S-105 | TIME FORMAT | String |
| S-106 | DATE FORMAT | String |
| S-107 | SCREEN SAVER | Int |
| S-108 | LANGUAGE | String |
| S-109 | TIME SYNC | String |
| S-110 | TIME SERVER | UserString |
| S-111 | SYNC PERIOD | String |
| S-112 | TIME OFFSET | Int |
| S-113 | TIME OFFSET 5 | Cent |
| S-114 | PEAK METER | String |
| S-115 | OUT METER SOURCE | String |
| S-116 | MB GR METER | String |
| S-117 | METER SLEEP | String |
| S-118 | RDS DYNAMIC PS | UserString |
| S-119 | RDS DPS SPEED | Int |
| S-120 | RDS DPS TIMEOUT | String |
| S-121 | RDS RADIO TEXT | UserString |
| S-122 | RDS RADIO TEXT SPEED | String |
| S-123 | RDS PROGRAM ID | UserString |
| S-124 | RDS PROGRAM TYPE | UserString |
| S-125 | RDS PROGRAM NAME | UserString |
| S-126 | RDS MUSIC SPEECH | String |
| S-127 | RDS DECODER INFO | String |
| S-128 | RDS TRAFFIC PROGRAM | String |
| S-129 | RDS TA TIMEOUT | Int |
| S-130 | RDS ALTERNATE FREQUENCY 1 | Int |
| S-131 | RDS ALTERNATE FREQUENCY 2 | Int |
| S-132 | RDS ALTERNATE FREQUENCY 3 | Int |
| S-133 | RDS ALTERNATE FREQUENCY 4 | Int |
| S-134 | RDS ALTERNATE FREQUENCY 5 | Int |
| S-135 | RDS ALTERNATE FREQUENCY 6 | Int |
| S-136 | RDS ALTERNATE FREQUENCY 7 | Int |
| S-137 | RDS ALTERNATE FREQUENCY 8 | Int |
| S-138 | RDS ALTERNATE FREQUENCY 9 | Int |
| S-139 | RDS ALTERNATE FREQUENCY 10 | Int |
| S-140 | RDS ALTERNATE FREQUENCY 11 | Int |
| S-141 | RDS ALTERNATE FREQUENCY 12 | Int |
| S-142 | RDS ALTERNATE FREQUENCY 13 | Int |
| S-143 | RDS ALTERNATE FREQUENCY 14 | Int |
| S-144 | RDS ALTERNATE FREQUENCY 15 | Int |
| S-145 | RDS ALTERNATE FREQUENCY 16 | Int |
| S-146 | RDS ALTERNATE FREQUENCY 17 | Int |
| S-147 | RDS ALTERNATE FREQUENCY 18 | Int |
| S-148 | RDS ALTERNATE FREQUENCY 19 | Int |
| S-149 | RDS ALTERNATE FREQUENCY 20 | Int |
| S-150 | RDS ALTERNATE FREQUENCY 21 | Int |
| S-151 | RDS ALTERNATE FREQUENCY 22 | Int |
| S-152 | RDS ALTERNATE FREQUENCY 23 | Int |
| S-153 | RDS ALTERNATE FREQUENCY 24 | Int |
| S-154 | RDS TIME | String |
| S-155 | RDS ACTIVE | String |
| S-156 | RDS LEVEL | Cent |
| S-157 | RDS PORT | Int |
| S-158 | RDS SOURCE IP | UserString |
| S-159 | RDS ECHO | String |
| S-160 | RDS HEADER | String |
| S-161 | RDS EAS TIME | Int |
| S-162 | RDS EAS | UserString |
| S-163 | RDS UECP PORT | Int |
| S-164 | RDS UECP SITE | Int |
| S-165 | RDS UECP ENCODE | Int |
| S-166 | RDS UECP ACTIVE | String |
| S-167 | RDS UECP UDP | String |
| S-168 | KANTAR ENABLE | String |
| S-169 | KANTAR CHANNEL_ID | UserString |
| S-170 | ACTUAL KANTAR | String |
| S-171 | RATINGS SELECT | String |
| S-172 | RATINGS EMBED | String |
| S-173 | RATINGS CSID | UserString |
| S-174 | RATINGS CBET CHECK DIGIT | UserString |
| S-175 | RATINGS PROCESS TYPE | String |
| S-176 | RATINGS CBET MODE | String |
| S-177 | RATINGS EAS MODE | String |
| S-178 | RATINGS STEPASIDE | String |

