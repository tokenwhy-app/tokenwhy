
export type Scheme = 'light' | 'dark';
export type Rules = 'soft' | 'glass';

export interface Theme { id: string; family: string; label?: string; scheme: Scheme; needs?: 'normal'; rules?: Rules }
export interface Family { id: string; label: string; light: Theme; dark: Theme; rules?: Rules }

export const THEMES: Theme[] = [
  { id: 'light', family: 'default', label: 'Default', scheme: 'light' },
  { id: 'dark', family: 'default', scheme: 'dark' },
  { id: 'neu', family: 'neu', label: 'Soft', scheme: 'light', rules: 'soft' },
  { id: 'neu-dark', family: 'neu', scheme: 'dark', rules: 'soft' },
  { id: 'clay', family: 'clay', label: 'Bubbly', scheme: 'light', rules: 'soft' },
  { id: 'clay-dark', family: 'clay', scheme: 'dark', rules: 'soft' },
  { id: 'pastel', family: 'pastel', label: 'Pink', scheme: 'light' },
  { id: 'pastel-dark', family: 'pastel', scheme: 'dark' },
  { id: 'green', family: 'green', label: 'Green', scheme: 'light' },
  { id: 'green-dark', family: 'green', scheme: 'dark' },
  { id: 'orange', family: 'orange', label: 'Orange', scheme: 'light' },
  { id: 'orange-dark', family: 'orange', scheme: 'dark' },
  { id: 'glass', family: 'glass', label: 'Glass', scheme: 'dark', needs: 'normal', rules: 'glass' },
];

export const FAMILIES: Family[] = [...new Set(THEMES.map((t) => t.family))].map((id) => {
  const vs = THEMES.filter((t) => t.family === id);
  const pick = (s: Scheme) => vs.find((t) => t.scheme === s) || vs[0];
  return { id, label: vs[0].label!, light: pick('light'), dark: pick('dark'), rules: vs[0].rules };
});

export const familyOf = (id: string): Family => FAMILIES.find((f) => f.id === id) || FAMILIES[0];
