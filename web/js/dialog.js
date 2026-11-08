// One reusable, themed dialog for the whole app (never alert/confirm/prompt). Closes on backdrop click, Escape or a [data-close] button.

let dialog = null;

function ensureDialog() {
  if (dialog) return dialog;
  dialog = document.createElement('dialog');
  dialog.className = 'app-dialog';
  dialog.innerHTML = `
    <div class="dialog-body">
      <div class="dialog-head">
        <h2 class="dialog-title"></h2>
        <button type="button" class="dialog-x" data-close><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 6l12 12M18 6 6 18"/></svg></button>
      </div>
      <div class="dialog-content"></div>
      <div class="dialog-foot"><button type="button" class="dialog-button" data-close></button></div>
    </div>`;
  // Clicks on the backdrop land on the <dialog> itself, because .dialog-body fills the whole box.
  dialog.addEventListener('click', event => {
    if (event.target === dialog || event.target.closest('[data-close]')) dialog.close();
  });
  document.body.append(dialog);
  return dialog;
}

/** content may be an HTML string or a DOM node. Returns the dialog element. */
export function showDialog({ title, content, closeLabel }) {
  const element = ensureDialog();
  element.querySelector('.dialog-title').textContent = title;
  const body = element.querySelector('.dialog-content');
  if (typeof content === 'string') body.innerHTML = content;
  else body.replaceChildren(content);
  element.querySelector('.dialog-button').textContent = closeLabel;
  element.querySelector('.dialog-x').setAttribute('aria-label', closeLabel);
  if (!element.open) element.showModal();
  return element;
}
