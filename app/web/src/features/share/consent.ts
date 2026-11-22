// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

// Bump CONSENT_VERSION with any change: the app records which version the user agreed to.
export const CONSENT_VERSION = 'draft-1';

export const CONSENT = {
  title: 'Compare your setup with other tokenwhy users?',
  paragraphs: [
    'If you turn this on, tokenwhy sends a small summary once a week and shows your numbers next to the median of everyone who shares. It stays off unless you turn it on, and tokenwhy works the same without it.',
    'Sent once a week, each number rounded into a range: your startup load and what it is made of, how many skills and memory files load, subagent and tool-output shares, compactions, token totals and cost, the model families and providers you used, and the top three languages of the files your agent touched.',
    'Never sent: project, file, skill or server names, paths, prompts, code, account or email. The server does not keep your IP address.',
    'A random number made on this computer links your weeks together so you can delete them. Delete them from Settings at any time; the server deletes each week after 13 weeks anyway.',
  ],
  see: 'See the exact data',
  on: 'Turn on',
  later: 'Not now',
};
