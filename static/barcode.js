// Barcode scanning via native BarcodeDetector API with OpenFoodFacts lookup.
// Decoded codes are handed to window.onBarcodeMatch (defined by the add sheet
// in feed.html): known barcodes show a one-tap log card, unknown ones fall
// back to openOffLookup() below, which prefills the add-food form.
// Security: all data from the external API is set via .value = (not innerHTML).

// hx-boost navigation re-executes this script — stop any stream a previous
// execution left live before clobbering the handle, or the track is orphaned
// and holds the camera until the tab closes.
if (window.barcodeStream) window.barcodeStream.getTracks().forEach(t => t.stop());
window.barcodeStream = null;
window.barcodeAnimFrame = null;
// Bumped on every stop/start: an in-flight getUserMedia or detect() that
// resolves after its session ended must not touch the camera or reschedule,
// otherwise a stale start leaks a live camera track and the next open fails
// until the page is reloaded.
window.barcodeSession = window.barcodeSession || 0;

async function startBarcodeScanner() {
  stopBarcodeScanner();
  const session = ++window.barcodeSession;
  const status = document.getElementById('scan-status');
  if (!('BarcodeDetector' in window)) {
    status.textContent = 'Camera scanning not supported here — enter the code below.';
    document.getElementById('manual-barcode-input').focus();
    return;
  }
  status.textContent = 'Hold the barcode inside the frame';
  try {
    const stream = await navigator.mediaDevices.getUserMedia({
      video: { facingMode: 'environment' }
    });
    if (session !== window.barcodeSession) {
      stream.getTracks().forEach(t => t.stop());
      return;
    }
    window.barcodeStream = stream;
    const video = document.getElementById('barcode-video');
    video.srcObject = stream;
    // autoplay is only honored reliably on the element's first use. A stop
    // during playback rejects with AbortError — that is an ordinary teardown,
    // not a camera failure, so it must not reach the catch below and paint
    // "Camera error" over the status line. The session check handles it.
    try {
      await video.play();
    } catch {
      /* fall through to the session check */
    }
    if (session !== window.barcodeSession) return;
    const detector = new BarcodeDetector({
      formats: ['ean_13', 'ean_8', 'upc_a', 'upc_e', 'code_128', 'code_39']
    });
    async function detect() {
      if (session !== window.barcodeSession) return;
      try {
        const codes = await detector.detect(video);
        if (session !== window.barcodeSession) return;
        if (codes.length > 0) {
          stopBarcodeScanner();
          window.onBarcodeMatch(codes[0].rawValue);
        } else {
          window.barcodeAnimFrame = requestAnimationFrame(detect);
        }
      } catch {
        // A detect() that throws after teardown must not re-arm the loop —
        // the entry guard above only runs on the next frame, by which point
        // the handle has already been written over a stopped session.
        if (session !== window.barcodeSession) return;
        window.barcodeAnimFrame = requestAnimationFrame(detect);
      }
    }
    window.barcodeAnimFrame = requestAnimationFrame(detect);
  } catch (err) {
    if (session === window.barcodeSession) {
      status.textContent = 'Camera error: ' + err.message + ' — enter the code below.';
    }
  }
}

function stopBarcodeScanner() {
  window.barcodeSession++;
  if (window.barcodeStream) {
    window.barcodeStream.getTracks().forEach(t => t.stop());
    window.barcodeStream = null;
  }
  if (window.barcodeAnimFrame) {
    cancelAnimationFrame(window.barcodeAnimFrame);
    window.barcodeAnimFrame = null;
  }
  // Clearing srcObject alone leaves the element playing out its dead stream;
  // pause first so the next open starts from a stopped element rather than
  // resuming into a frozen last frame.
  const video = document.getElementById('barcode-video');
  if (video && video.srcObject) {
    video.pause();
    video.srcObject = null;
  }
}

function lookupManualBarcode() {
  const input = document.getElementById('manual-barcode-input');
  const barcode = input.value.trim();
  if (!barcode) return;
  input.value = '';
  window.onBarcodeMatch(barcode);
}

// Unknown product: prefill the library's add-food form from OpenFoodFacts
// and reveal it, so the user can save the item (then scan again to log it).
async function openOffLookup(barcode) {
  // Not a bare `form.hidden = false`: the form sits inside the food library's
  // collapsible body, so revealing it means opening that too — otherwise this
  // un-hides a form inside a `display: none` ancestor and scrolls to nothing.
  // feed.html owns that knowledge and exposes it as revealAddFoodForm().
  const form = typeof revealAddFoodForm === 'function'
    ? revealAddFoodForm()
    : document.getElementById('add-food-form');
  if (!form) return;
  form.hidden = false;
  form.scrollIntoView({ behavior: 'smooth', block: 'start' });
  try {
    const resp = await fetch(
      'https://world.openfoodfacts.org/api/v0/product/' + encodeURIComponent(barcode) + '.json'
    );
    const data = await resp.json();
    if (data.status !== 1) {
      // Product not found — just fill in the barcode, user enters the rest
      setField(form, 'barcode', barcode);
      return;
    }
    const p = data.product;
    const n = p.nutriments || {};
    setField(form, 'barcode', barcode);
    setField(form, 'name', p.product_name || p.product_name_en || '');
    setField(form, 'brand', p.brands || '');
    setField(form, 'calories', roundNutrient(n['energy-kcal_100g'] ?? n['energy-kcal'] ?? 0));
    setField(form, 'protein', roundNutrient(n['proteins_100g'] ?? 0));
    setField(form, 'carbs', roundNutrient(n['carbohydrates_100g'] ?? 0));
    setField(form, 'fat', roundNutrient(n['fat_100g'] ?? 0));
    setField(form, 'fiber', roundNutrient(n['fiber_100g'] ?? 0));
    setField(form, 'sugar', roundNutrient(n['sugars_100g'] ?? 0));
    setField(form, 'sodium', roundNutrient((n['sodium_100g'] ?? 0) * 1000)); // convert g to mg
    setField(form, 'saturated_fat', roundNutrient(n['saturated-fat_100g'] ?? 0));
    // Package size from product quantity (e.g. "565" for a 565g pizza)
    const pkgSize = parseFloat(p.product_quantity) || parseFloat(p.serving_quantity) || 0;
    if (pkgSize > 0) setField(form, 'package_size', pkgSize);
    // Image URL (from OpenFoodFacts CDN — safe external URL, not user-generated)
    const imgUrl = p.image_front_url || p.image_url || '';
    const imgField = document.getElementById('image-url-field');
    if (imgField) imgField.value = imgUrl;
  } catch (err) {
    setField(form, 'barcode', barcode);
  }
}

function setField(form, name, value) {
  const el = form.querySelector('[name="' + name + '"]');
  if (el) el.value = value;
}

function roundNutrient(v) {
  return Math.round((+v || 0) * 10) / 10;
}
