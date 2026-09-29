export const setupGroupNames = [
  'Audio Input',
  'Audio Outputs',
  'FM Transmission',
  'HD & Diversity',
  'Network & Time',
  'Remote Control',
  'RDS',
  'SNMP',
  'Display',
  'Identification & Ratings',
] as const;

export type SetupGroupName = typeof setupGroupNames[number];
export type SetupFieldKind = 'setting' | 'status' | 'routing';
export type SetupSection<T> = { name: string; fields: [string, T][] };
export type SetupGroup<T> = {
  name: SetupGroupName;
  description: string;
  fields: [string, T][];
  sections: SetupSection<T>[];
};

const groupDescriptions: Record<SetupGroupName, string> = {
  'Audio Input': 'Choose the programme input, reference levels and automatic fallback behaviour.',
  'Audio Outputs': 'Set output levels, formats and monitoring. Source assignment remains in Outputs.',
  'FM Transmission': 'Configure the FM carrier, stereo generator, pre-emphasis and modulation limits.',
  'HD & Diversity': 'Configure HD output behaviour, loudness protection and diversity delay.',
  'Network & Time': 'Manage network addressing, service ports, clock synchronisation and daylight saving.',
  'Remote Control': 'Configure automation contacts, tallies, security and external control interfaces.',
  'RDS': 'Manage programme identification, dynamic text, alternate frequencies and UECP transport.',
  'SNMP': 'Enable SNMP and configure manager addresses, ports and community access.',
  'Display': 'Choose front-panel behaviour and the meters shown on the processor display.',
  'Identification & Ratings': 'Set station identification and audience-measurement integrations.',
};

const sectionOrder: Record<SetupGroupName, string[]> = {
  'Audio Input': ['Source Selection', 'Analog Input', 'Digital Input', 'AES67 Input', 'Failover & Silence Detection'],
  'Audio Outputs': ['Analog Output', 'Digital Output 1', 'Digital Output 2', 'AES67 Output', 'Monitoring & Composite'],
  'FM Transmission': ['Operating Mode', 'Test & Bypass', 'Stereo Generator', 'Modulation & Carrier', 'Loudness Protection', 'Processing Structure'],
  'HD & Diversity': ['HD Output', 'Diversity Delay', 'Loudness Protection', 'HD Processing'],
  'Network & Time': ['IP Configuration', 'Service Ports', 'Clock & Calendar', 'Automatic Clock Set'],
  'Remote Control': ['Automation & Tallies', 'Remote Contacts', 'Security', 'External Interfaces'],
  'RDS': ['Programme Service', 'Dynamic Text', 'Alternate Frequencies', 'UECP & Network', 'Emergency Alerting'],
  'SNMP': ['Service', 'Manager Destinations', 'Community Access'],
  'Display': ['Front Panel', 'Meter Display'],
  'Identification & Ratings': ['Station & Hardware', 'Kantar', 'Ratings Encoder'],
};

// Model-specific fields of the 5500 and 8700HD. Checked first so the rules
// below keep placing every 5700i field exactly as before.
const TEST_AND_BYPASS = /^(TEST (MODE|TONE|BYPASS|CLIP DEFEAT|MODULATION|400HZ TONE)|XTALK TEST|MUTE|OPERATE)$/;
const PROCESSING_STRUCTURE = /^(2B SWITCH|5B SWITCH|2B\/5B SWITCH|STD SWITCH|ULL SWITCH|MX SWITCH|PASSTHRU SW|DRIVE W\d|MIX W\d|CLIP W\d|CLIP DENS THR|AGC MASTER TH|PHASE CORRECT(OR| DEFEAT| XOVER)|SUBHARMONIC|MAG PHASE COMP|B4\/5 DELTA REL)$/;
const MODULATION_EXTRAS = /^(DIGITAL SCA[12] LEVEL|COMPOSITE OSCOMP|MAIN OSCOMP|MPX PWR B5CTRL)$/;
const LOUDNESS_EXTRAS = /^(MPX PWR (REL|SP) CTRL|RESET ITU412)$/;
// Lower-case "hd" names are PC Remote's internal copies of the HD processing chain.
const HD_PROCESSING_COPY = /^hd /;
const AUTOMATIC_CLOCK = /^(AUTO SET |SET BY |(SUN|MON|TUES|WEDNES|THURS|FRI|SATUR)DAY$|CLOCK CONTROL$)/;
const MANUAL_CLOCK = /^SET (HOUR|MINUTE|SECOND|DAY|MONTH|YEAR)$/;

function modelSpecificGroup(name: string): SetupGroupName | null {
  if (/^(EI[12] |INPUT LEVEL$)/.test(name)) return 'Audio Input';
  if (/^(EO[12] |EO LR SWAP|DO LR SWAP|DIGITAL COMP LEVEL$)/.test(name)) return 'Audio Outputs';
  if (TEST_AND_BYPASS.test(name) || PROCESSING_STRUCTURE.test(name) || MODULATION_EXTRAS.test(name) || LOUDNESS_EXTRAS.test(name)) return 'FM Transmission';
  if (HD_PROCESSING_COPY.test(name)) return 'HD & Diversity';
  if (AUTOMATIC_CLOCK.test(name) || MANUAL_CLOCK.test(name)) return 'Network & Time';
  if (/^(PASSCODE ACCESS LEVEL|CURRENT PASSCODES)$/.test(name)) return 'Remote Control';
  if (/^(METER OPTION|AGC METER)$/.test(name)) return 'Display';
  return null;
}

const IDENTIFICATION = /^(STATION ID|KANTAR|ACTUAL KANTAR|RATINGS|SERIAL|HARDWARE|FIRMWARE|VERSION)/;

/** Fields no rule recognizes; they fall back to Identification & Ratings. */
export function unrecognizedSetupFields(names: string[]): string[] {
  return names.filter(name => setupGroup(name) === 'Identification & Ratings' && !IDENTIFICATION.test(name));
}

function setupGroup(name: string): SetupGroupName {
  const specific = modelSpecificGroup(name);
  if (specific) return specific;
  if (/^RDS /.test(name)) return 'RDS';
  if (/SNMP/.test(name)) return 'SNMP';
  if (/^(INPUT A OR D|ACTUAL A OR D|AI |DI |ANALOG FALLBACK|DIGITAL FALLBACK|SILENCE )/.test(name)) return 'Audio Input';
  if (/^(AO|DO[12] |PHONES OUT|OUT METER SOURCE|Monitor Mute|COMP[12] OUT|AO PRE-OUT)/.test(name)) return 'Audio Outputs';
  if (/^(ALGORITHM|BYPASS GAIN|PILOT|PRE-E|FM POLARITY|FREQUENCY|MODULATION|MOD |MOD REDUC|CLIP DEFEAT|TEST PILOT|FM BS1770|ITU412)/.test(name)) return 'FM Transmission';
  if (/^(HD |DIVERSITY|IBOC |BS1770)/.test(name)) return 'HD & Diversity';
  if (/^(NETWORK|TERMINAL PORT|TIME |DATE FORMAT|DAYLIGHT|STANDARD MONTH|SYNC PERIOD)/.test(name)) return 'Network & Time';
  if (/^(AUTOMATION|REMOTE CONTACT|TALLY|SECURITY|MODEM INIT STRING|STUDIO CHASSIS|INTERFACE TYPE)/.test(name)) return 'Remote Control';
  if (/^(CONTRAST|LANGUAGE|LDNES METER|MB GR METER|METER SLEEP|PEAK METER|SCREEN SAVER|SHOW IP|VIEW METERS)/.test(name)) return 'Display';
  return 'Identification & Ratings';
}

function setupSection(group: SetupGroupName, name: string): string {
  switch (group) {
    case 'Audio Input':
      if (/^EI[12] /.test(name)) return 'AES67 Input';
      if (/^AI /.test(name)) return 'Analog Input';
      if (/^DI /.test(name)) return 'Digital Input';
      if (/FALLBACK|SILENCE/.test(name)) return 'Failover & Silence Detection';
      return 'Source Selection';
    case 'Audio Outputs':
      if (/^(EO[12] |EO LR SWAP)/.test(name)) return 'AES67 Output';
      if (/^(DO1 |DO LR SWAP)/.test(name)) return 'Digital Output 1';
      if (/^DO2 /.test(name)) return 'Digital Output 2';
      if (/^(PHONES|OUT METER|Monitor|COMP|DIGITAL COMP)/.test(name)) return 'Monitoring & Composite';
      return 'Analog Output';
    case 'FM Transmission':
      if (TEST_AND_BYPASS.test(name)) return 'Test & Bypass';
      if (PROCESSING_STRUCTURE.test(name)) return 'Processing Structure';
      if (LOUDNESS_EXTRAS.test(name)) return 'Loudness Protection';
      if (/^(ALGORITHM|BYPASS)/.test(name)) return 'Operating Mode';
      if (/^(PILOT|PRE-E|FM POLARITY)/.test(name)) return 'Stereo Generator';
      if (/FM BS1770|ITU412/.test(name)) return 'Loudness Protection';
      return 'Modulation & Carrier';
    case 'HD & Diversity':
      if (HD_PROCESSING_COPY.test(name)) return 'HD Processing';
      if (/DIVERSITY/.test(name)) return 'Diversity Delay';
      if (/BS1770|ITU412/.test(name)) return 'Loudness Protection';
      return 'HD Output';
    case 'Network & Time':
      if (/^NETWORK (?!PORT)/.test(name)) return 'IP Configuration';
      if (/PORT$/.test(name)) return 'Service Ports';
      if (AUTOMATIC_CLOCK.test(name)) return 'Automatic Clock Set';
      return 'Clock & Calendar';
    case 'Remote Control':
      if (/^REMOTE CONTACT/.test(name)) return 'Remote Contacts';
      if (/^(AUTOMATION|TALLY)/.test(name)) return 'Automation & Tallies';
      if (/^(SECURITY|PASSCODE|CURRENT PASSCODES)/.test(name)) return 'Security';
      return 'External Interfaces';
    case 'RDS':
      if (/ALTERNATE FREQUENCY/.test(name)) return 'Alternate Frequencies';
      if (/UECP|RDS PORT|SOURCE IP/.test(name)) return 'UECP & Network';
      if (/EAS/.test(name)) return 'Emergency Alerting';
      if (/DPS|DYNAMIC PS|RADIO TEXT/.test(name)) return 'Dynamic Text';
      return 'Programme Service';
    case 'SNMP':
      if (/ADDRESS|PORT/.test(name)) return 'Manager Destinations';
      if (/READ|WRITE/.test(name)) return 'Community Access';
      return 'Service';
    case 'Display':
      return /METER/.test(name) ? 'Meter Display' : 'Front Panel';
    case 'Identification & Ratings':
      if (/^KANTAR|^ACTUAL KANTAR/.test(name)) return 'Kantar';
      if (/^RATINGS/.test(name)) return 'Ratings Encoder';
      return 'Station & Hardware';
  }
}

const exactLabels: Record<string, string> = {
  'ALGORITHM': 'Operating Mode',
  'INPUT A OR D': 'Preferred Input',
  'ACTUAL A OR D': 'Active Input',
  'ANALOG FALLBACK': 'Allow Analog Fallback',
  'DIGITAL FALLBACK': 'Allow Digital Fallback',
  'DI ANALOG FALLBACK': 'Digital Input Analog Fallback',
  'AO PRE-OUT': 'Analog Pre-processed Output',
  'Monitor Mute': 'Mute Monitor Output',
  'START SNMP': 'SNMP Service',
  'SECURITY ACTIVE': 'Remote Security',
  'STATION ID': 'Station Name',
  'RDS PROGRAM ID': 'Programme Identification (PI)',
  'RDS PROGRAM NAME': 'Program Service Name',
  'RDS PROGRAM TYPE': 'Programme Type (PTY)',
  'RDS DYNAMIC PS': 'Dynamic Program Service',
  'RDS RADIO TEXT': 'RadioText',
  'RDS SOURCE IP': 'UECP Source Address',
  'ACTUAL KANTAR': 'Kantar Status',
  'LDNES METER UNITS': 'Loudness Meter Units',
  'IBOC METER': 'HD Meter Source',
};

export function setupLabel(name: string): string {
  if (exactLabels[name]) return exactLabels[name];
  return name
    .replace(/^AI /, 'Analog Input ')
    .replace(/^DI /, 'Digital Input ')
    .replace(/^AO1 /, 'Analog Output ')
    .replace(/^DO1 /, 'Digital Output 1 ')
    .replace(/^DO2 /, 'Digital Output 2 ')
    .replace(/^RDS /, '')
    .replace(/_/g, ' ')
    .replace(/\bREF\b/g, 'Reference')
    .replace(/\bTHR\b/g, 'Threshold')
    .replace(/\bLDNES\b/g, 'Loudness')
    .replace(/\bPRE EMPH\b/g, 'Pre-emphasis')
    .replace(/\bWORD LENGTH\b/g, 'Word Length')
    .replace(/\bBW\b/g, 'Bandwidth')
    .replace(/\bDPS\b/g, 'Dynamic PS')
    .replace(/\bBEGINS\b/g, 'Starts')
    .toLowerCase()
    .replace(/\b\p{L}/gu, letter => letter.toUpperCase())
    .replace(/\bFm\b/g, 'FM')
    .replace(/\bHd\b/g, 'HD')
    .replace(/\bRds\b/g, 'RDS')
    .replace(/\bSnmp\b/g, 'SNMP')
    .replace(/\bUecp\b/g, 'UECP')
    .replace(/\bIp\b/g, 'IP')
    .replace(/\bId\b/g, 'ID')
    .replace(/\bCbet\b/g, 'CBET')
    .replace(/\bCsid\b/g, 'CSID')
    .replace(/\bPpm\b/g, 'PPM')
    .replace(/\bEas\b/g, 'EAS');
}

export function setupCurrentValue(value: string): string {
  const normalized = value.trim().toLocaleLowerCase();
  return !normalized || normalized === 'undefined' || normalized === 'null' ? 'Not Configured' : value;
}

export function setupFieldKind(name: string): SetupFieldKind {
  if (/^ACTUAL /.test(name)) return 'status';
  if (['AO1 SOURCE', 'DO1 SOURCE', 'DO2 SOURCE', 'PHONES OUT SOURCE'].includes(name)) return 'routing';
  return 'setting';
}

export function groupSetupFields<T>(fields: Record<string, T>): SetupGroup<T>[] {
  const grouped = new Map<SetupGroupName, [string, T][]>(setupGroupNames.map(name => [name, []]));
  Object.entries(fields).sort(([a],[b])=>setupLabel(a).localeCompare(setupLabel(b))).forEach(entry => grouped.get(setupGroup(entry[0]))!.push(entry));
  return setupGroupNames.map(name => {
    const groupFields = grouped.get(name)!;
    const sections = sectionOrder[name].map(sectionName => ({
      name: sectionName,
      fields: groupFields.filter(([fieldName]) => setupSection(name, fieldName) === sectionName),
    })).filter(section => section.fields.length > 0);
    return {name,description:groupDescriptions[name],fields:groupFields,sections};
  }).filter(group => group.fields.length > 0);
}
