// The .dc.html boots through support.js, which pulls React from a CDN. This
// writes a plain-HTML twin of the same markup — same design-system stylesheet,
// no canvas runtime — so the layout can be rendered and checked offline.
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
const here = dirname(fileURLToPath(import.meta.url));
let s = readFileSync(join(here, 'Van loading board.dc.html'), 'utf8');
s = s.replace('<script src="./support.js"></script>',
  '<link rel="stylesheet" href="./_ds/hampter-design-system-03c25988-bba8-4fc5-801f-653a333b24c3/styles.css">');
s = s.replace(/<helmet>[\s\S]*?<\/helmet>/, '<style>body{margin:0;background:var(--surface-void)}</style>');
s = s.replace('<x-dc>', '').replace('</x-dc>', '');
writeFileSync(join(here, '.preview.html'), s);
console.log('wrote .preview.html');
