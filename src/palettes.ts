import type { PaletteKey } from './types';

export interface Palette {
  name: string;
  bg: string;
  sidebar: string;
  sidebarFlat: string;
  panel: string;
  line: string;
  lineSoft: string;
  text: string;
  text2: string;
  text3: string;
  rowHover: string;
  rowSel: string;
  accent: string;
  accentSoft: string;
  accentDark: string;
  hub: string;
  hubSoft: string;
  hubText: string;
  danger: string;
  iconBg: string;
  iconLg: string;
  iconTextLg: string;
}

export const PALETTES: Record<PaletteKey, Palette> = {
  cool: {
    name: 'Cool · Stock macOS',
    bg: '#f5f5f7',
    sidebar: 'linear-gradient(180deg, rgba(232,232,237,.92), rgba(222,224,232,.88))',
    sidebarFlat: '#ebebf0',
    panel: '#ffffff',
    line: 'rgba(0,0,0,0.08)',
    lineSoft: 'rgba(0,0,0,0.05)',
    text: '#1d1d1f',
    text2: '#6e6e73',
    text3: '#86868b',
    rowHover: 'rgba(0,0,0,0.035)',
    rowSel: 'rgba(10,132,255,0.10)',
    accent: '#0a84ff',
    accentSoft: 'rgba(10,132,255,0.10)',
    accentDark: '#1d1d1f',
    hub: '#34c759',
    hubSoft: 'rgba(52,199,89,0.16)',
    hubText: '#1f7a3a',
    danger: '#ff3b30',
    iconBg: 'linear-gradient(135deg, #e5e5ea, #d1d1d6)',
    iconLg: 'linear-gradient(135deg, #d2d2d7, #b0b0b5)',
    iconTextLg: '#ffffff',
  },
  warm: {
    name: 'Warm · Editorial',
    bg: '#faf8f3',
    sidebar: 'linear-gradient(180deg, rgba(241,237,226,1), rgba(236,231,217,1))',
    sidebarFlat: '#f1ede2',
    panel: '#fbf9f4',
    line: 'rgba(40,32,20,0.10)',
    lineSoft: 'rgba(40,32,20,0.06)',
    text: '#1c1814',
    text2: '#6b6358',
    text3: '#9c948a',
    rowHover: 'rgba(40,32,20,0.03)',
    rowSel: 'rgba(122,75,43,0.10)',
    accent: '#7a4b2b',
    accentSoft: 'rgba(122,75,43,0.10)',
    accentDark: '#3a2415',
    hub: '#5b7a4a',
    hubSoft: 'rgba(91,122,74,0.16)',
    hubText: '#3a5230',
    danger: '#a04020',
    iconBg: 'linear-gradient(135deg, #e8e0cc, #d4c9b0)',
    iconLg: 'linear-gradient(135deg, #c4a888, #8a6a4a)',
    iconTextLg: '#ffffff',
  },
  slate: {
    name: 'Slate · Muted neutral',
    bg: '#f3f4f6',
    sidebar: 'linear-gradient(180deg, rgba(226,229,235,1), rgba(216,220,228,1))',
    sidebarFlat: '#e2e5eb',
    panel: '#ffffff',
    line: 'rgba(15,23,42,0.10)',
    lineSoft: 'rgba(15,23,42,0.06)',
    text: '#0f172a',
    text2: '#475569',
    text3: '#94a3b8',
    rowHover: 'rgba(15,23,42,0.03)',
    rowSel: 'rgba(56,82,126,0.10)',
    accent: '#38527e',
    accentSoft: 'rgba(56,82,126,0.10)',
    accentDark: '#1e293b',
    hub: '#0f766e',
    hubSoft: 'rgba(15,118,110,0.14)',
    hubText: '#0d5752',
    danger: '#b91c1c',
    iconBg: 'linear-gradient(135deg, #e2e8f0, #cbd5e1)',
    iconLg: 'linear-gradient(135deg, #94a3b8, #475569)',
    iconTextLg: '#ffffff',
  },
};

export const PALETTE_OPTIONS: string[][] = [
  ['#0a84ff', '#ebebf0', '#ffffff', '#34c759'],
  ['#7a4b2b', '#f1ede2', '#faf8f3', '#5b7a4a'],
  ['#38527e', '#e2e5eb', '#ffffff', '#0f766e'],
];

export const PALETTE_KEYS: PaletteKey[] = ['cool', 'warm', 'slate'];
