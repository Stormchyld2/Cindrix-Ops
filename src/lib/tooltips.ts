// Hover text for every button, in plain words. A button that already has its own `title` keeps it.
// For the rest, the words below are used when the button's label is known; otherwise the label
// itself is turned into a short sentence ("Press to: Open Music"). New buttons are covered
// automatically, so nothing is left without a hint. See docs/11-navigation.md.

const KNOWN: Record<string, string> = {
  save: 'Save your changes to this drive',
  'save item': 'Save this item to your inventory',
  'save result': 'Keep your score for this lesson',
  'save to this drive': 'Keep a copy of this on the drive',
  'save this website': 'Copy this website onto the drive so you can read it without internet',
  'save as pdf': 'Open the print window; choose "Save as PDF" there',
  print: 'Print this page',
  add: 'Add it to the list',
  'add note': 'Keep this note with the book',
  'add corner': 'Add this corner to the area you are drawing',
  'add a person': 'Add another person to the family page',
  'add a folder…': 'Choose a folder on this computer to include',
  '+ add archive': 'Add an archive file (a saved library of pages) to the drive',
  cancel: 'Close this without changing anything',
  keep: 'Do not delete it',
  remove: 'Take this out of the list',
  delete: 'Delete this (you will be asked to confirm)',
  'yes, delete': 'Yes, delete it for good',
  'delete selected': 'Delete the items you ticked (you will be asked to confirm)',
  'delete it now': 'Delete it now, for good',
  'really delete?': 'Press again to delete for good',
  edit: 'Change the details of this one',
  find: 'Look this up',
  search: 'Search for these words',
  send: 'Send your question',
  ok: 'Close this message',
  unlock: 'Open the page with your passphrase',
  'lock this page': 'Lock this page again so it needs the passphrase',
  'set a passphrase': 'Choose a passphrase that protects this page',
  'turn the lock off': 'Remove the passphrase from this page',
  'new map': 'Start a fresh, empty map',
  'export route (gpx)': 'Write the route to a file a phone map app can open',
  'finish area': 'Close the area from the corners you have added',
  'pin it': 'Keep the place you found as a pin',
  'zoom to my points': 'Zoom the map so all your pins and areas fit',
  'full screen': 'Make the map fill the whole screen',
  'exit full screen': 'Go back to the normal page',
  'start over': 'Throw these away and begin again',
  'clear route': 'Remove the route but keep the pins',
  'add to route': 'Add this pin to the end of your route',
  'go to': 'Move the map to this one',
  'look again': 'Look for drives and files again',
  'show more': 'Show more of the list',
  'take the quiz': 'Answer a few questions about this lesson',
  'check answers': 'See how many you got right',
  'try again': 'Start this lesson’s quiz over',
  'next lesson →': 'Go to the next lesson',
  '← all courses': 'Go back to the list of courses',
  '← back to the course': 'Go back to this course',
  '← back': 'Go back',
  back: 'Go back',
  'new tab': 'Open another page in Intel',
  'open archives': 'Go to the list of archives',
  'open inventory': 'Go to your supplies list',
  'open the emergency card': 'Open the checked emergency card this answer came from',
  resume: 'Carry on from where you stopped reading',
  next: 'Next',
  previous: 'Previous',
  shuffle: 'Play in a random order',
  '⤮ shuffle': 'Play everything in a random order',
  '▶ play': 'Play this playlist',
  'always use it': 'Read this drive now and every time it is plugged in',
  'never use it': 'Never read this drive',
  'just this time': 'Read this drive this once, then ask again next time',
  'ask me again': 'Forget the answer and ask again next time',
  'turn on': 'Turn this on',
  'turn off': 'Turn this off',
  share: 'Let other devices on your network read the library (read-only)',
  night: 'Dim the page for reading in the dark',
  'reset every setting': 'Put every setting back to how it came',
  'put these back to how they came': 'Put the settings in this box back to how they came',
  'remove from drive': 'Delete this file from the drive (you will be asked to confirm)',
  'stop and discard': 'Stop and throw away what was collected',
  'end drill without saving': 'Stop this drill and do not record it',
  'make a map': 'Make a map',
  'smaller text': 'Make the text smaller',
  'larger text': 'Make the text bigger',
  'a＋': 'Make the text bigger',
  'intel home': 'Go to the Intel home page',
  '⌂': 'Go to the Intel home page',
  '✕': 'Close or remove this',
  '＋': 'Open another page',
  '←': 'Go back',
  '→': 'Go forward',
  '↑': 'Move up',
  '↓': 'Move down',
  '−': 'Take one away',
  '+': 'Add one',
  '⏭': 'Next song',
  '⏮': 'Previous song',
  '‹ prev': 'Previous page',
  'next ›': 'Next page',
  '‹ shelf': 'Back to the shelf',
};

/** Words for a button, from what it says. */
export function tipFor(label: string, aria: string): string {
  const key = (label || '').replace(/\s+/g, ' ').trim().toLowerCase();
  const a = (aria || '').replace(/\s+/g, ' ').trim();
  if (KNOWN[key]) return KNOWN[key];
  if (a && KNOWN[a.toLowerCase()]) return KNOWN[a.toLowerCase()];
  if (a) return a;
  if (!key) return '';
  if (key.length > 60) return '';
  return 'Press to: ' + (label.replace(/\s+/g, ' ').trim());
}

const SELECTOR = 'button, [role="button"], summary, input[type="submit"], input[type="button"]';

function apply(el: Element): void {
  if (!(el instanceof HTMLElement)) return;
  const auto = el.hasAttribute('data-autotip');
  if (el.hasAttribute('title') && !auto) return; // the author wrote one
  const label = el instanceof HTMLInputElement ? el.value : (el.textContent ?? '');
  const tip = tipFor(label, el.getAttribute('aria-label') ?? '');
  if (!tip) return;
  if (el.getAttribute('title') !== tip) el.setAttribute('title', tip);
  el.setAttribute('data-autotip', '1');
}

function sweep(root: ParentNode): void {
  if (root instanceof Element && root.matches(SELECTOR)) apply(root);
  root.querySelectorAll?.(SELECTOR).forEach(apply);
}

/** Remove the hints this file added (the ones an author wrote stay). */
export function removeTooltips(): void {
  if (typeof document === 'undefined') return;
  document.querySelectorAll('[data-autotip]').forEach((el) => {
    el.removeAttribute('title');
    el.removeAttribute('data-autotip');
  });
}

/** Give every button hover text now and whenever the page changes. Returns a function that stops it. */
export function installTooltips(): () => void {
  if (typeof document === 'undefined' || typeof MutationObserver === 'undefined') return () => {};
  sweep(document.body);
  const mo = new MutationObserver((list) => {
    for (const m of list) {
      if (m.type === 'childList') {
        m.addedNodes.forEach((n) => { if (n instanceof Element) sweep(n); });
      }
      // a label changed (for example "Delete" became "Really delete?"): refresh the hint of the button around it
      const t = m.target instanceof Element ? m.target : m.target.parentElement;
      const b = t?.closest(SELECTOR);
      if (b) apply(b);
    }
  });
  mo.observe(document.body, { childList: true, subtree: true, characterData: true, attributes: true, attributeFilter: ['aria-label'] });
  return () => mo.disconnect();
}
