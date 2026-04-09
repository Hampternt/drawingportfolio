// Barcode scanning via native BarcodeDetector API with OpenFoodFacts lookup.
// Security: all data from external API is set via .value = (not innerHTML).

let barcodeStream = null;
let barcodeAnimFrame = null;

async function startBarcodeScanner(formId) {
  if (!('BarcodeDetector' in window)) {
    document.getElementById('manual-barcode').hidden = false;
    document.getElementById('manual-barcode-input').focus();
    return;
  }
  const scanner = document.getElementById('barcode-scanner');
  scanner.hidden = false;
  document.getElementById('scan-status').textContent = 'Scanning…';
  try {
    barcodeStream = await navigator.mediaDevices.getUserMedia({
      video: { facingMode: 'environment' }
    });
    const video = document.getElementById('barcode-video');
    video.srcObject = barcodeStream;
    const detector = new BarcodeDetector({
      formats: ['ean_13', 'ean_8', 'upc_a', 'upc_e', 'code_128', 'code_39']
    });
    async function detect() {
      try {
        const codes = await detector.detect(video);
        if (codes.length > 0) {
          stopBarcodeScanner();
          await lookupBarcode(codes[0].rawValue, formId);
        } else {
          barcodeAnimFrame = requestAnimationFrame(detect);
        }
      } catch {
        barcodeAnimFrame = requestAnimationFrame(detect);
      }
    }
    barcodeAnimFrame = requestAnimationFrame(detect);
  } catch (err) {
    document.getElementById('scan-status').textContent = 'Camera error: ' + err.message;
  }
}

function stopBarcodeScanner() {
  if (barcodeStream) {
    barcodeStream.getTracks().forEach(t => t.stop());
    barcodeStream = null;
  }
  if (barcodeAnimFrame) {
    cancelAnimationFrame(barcodeAnimFrame);
    barcodeAnimFrame = null;
  }
  document.getElementById('barcode-scanner').hidden = true;
}

async function lookupManualBarcode() {
  const input = document.getElementById('manual-barcode-input');
  const barcode = input.value.trim();
  if (!barcode) return;
  document.getElementById('manual-barcode').hidden = true;
  await lookupBarcode(barcode, 'add-food-form');
}

async function lookupBarcode(barcode, formId) {
  const status = document.getElementById('scan-status');
  document.getElementById('add-food-form').hidden = false;
  const form = document.getElementById(formId);
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
