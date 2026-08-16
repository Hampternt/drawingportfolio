// Encode ArrayBuffer to Base64url string
function bufToB64url(buf) {
  return btoa(String.fromCharCode(...new Uint8Array(buf)))
    .replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
}

// Decode Base64url string to Uint8Array
function b64urlToBuf(str) {
  const b64 = str.replace(/-/g, '+').replace(/_/g, '/');
  const bin = atob(b64);
  return Uint8Array.from(bin, c => c.charCodeAt(0));
}

async function startLogin() {
  const status = document.getElementById('status');
  status.textContent = '';

  try {
    // Step 1: Get challenge from server
    const startResp = await fetch('/api/auth/login/start', { method: 'POST' });
    const { challenge_id, options } = await startResp.json();

    // Decode challenge and allowCredentials buffers
    options.publicKey.challenge = b64urlToBuf(options.publicKey.challenge);
    if (options.publicKey.allowCredentials) {
      options.publicKey.allowCredentials = options.publicKey.allowCredentials.map(c => ({
        ...c, id: b64urlToBuf(c.id)
      }));
    }

    // Step 2: Ask device for passkey
    const credential = await navigator.credentials.get(options);

    // Step 3: Encode response and send to server
    const credJson = {
      id: credential.id,
      rawId: bufToB64url(credential.rawId),
      type: credential.type,
      response: {
        clientDataJSON: bufToB64url(credential.response.clientDataJSON),
        authenticatorData: bufToB64url(credential.response.authenticatorData),
        signature: bufToB64url(credential.response.signature),
        userHandle: credential.response.userHandle
          ? bufToB64url(credential.response.userHandle) : null,
      },
    };

    const finishResp = await fetch('/api/auth/login/finish', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ challenge_id, credential: credJson }),
    });
    const result = await finishResp.json();

    if (result.ok) {
      window.location.href = '/admin';
    } else {
      status.textContent = result.error || 'Login failed';
    }
  } catch (err) {
    status.textContent = err.message || 'Login failed';
  }
}

async function startRegister() {
  const status = document.getElementById('status');
  status.textContent = '';

  try {
    const startResp = await fetch('/api/auth/register/start', { method: 'POST' });
    const { challenge_id, options } = await startResp.json();

    options.publicKey.challenge = b64urlToBuf(options.publicKey.challenge);
    options.publicKey.user.id = b64urlToBuf(options.publicKey.user.id);
    if (options.publicKey.excludeCredentials) {
      options.publicKey.excludeCredentials = options.publicKey.excludeCredentials.map(c => ({
        ...c, id: b64urlToBuf(c.id)
      }));
    }

    const credential = await navigator.credentials.create(options);

    const credJson = {
      id: credential.id,
      rawId: bufToB64url(credential.rawId),
      type: credential.type,
      response: {
        clientDataJSON: bufToB64url(credential.response.clientDataJSON),
        attestationObject: bufToB64url(credential.response.attestationObject),
      },
    };

    const finishResp = await fetch('/api/auth/register/finish', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ challenge_id, credential: credJson }),
    });
    const result = await finishResp.json();

    if (result.ok) {
      status.textContent = 'Passkey registered! You can now log in.';
      status.style.color = 'green';
    } else {
      status.textContent = result.error || 'Registration failed';
    }
  } catch (err) {
    status.textContent = err.message || 'Registration failed';
  }
}

// ── Name + PIN login ─────────────────────────────────────────────────────────
//
// The alternative to the passkey ceremony above, for fitness accounts on a
// device that holds no credential. Lives in this file rather than its own
// because the login page loads exactly one script and both paths write to the
// same #status element.
async function startPinLogin(event) {
  event.preventDefault();
  const status = document.getElementById('status');
  const btn = document.getElementById('pin-btn');
  const name = document.getElementById('pin-name').value;
  const pin = document.getElementById('pin-pin').value;

  status.style.color = '';
  status.textContent = 'Checking…';
  // Argon2 verification is deliberately slow, so the button has to stop
  // accepting clicks or an impatient double-tap spends two of the five
  // attempts before the first answer arrives.
  btn.disabled = true;

  try {
    const resp = await fetch('/api/auth/login/pin', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name, pin }),
    });
    const result = await resp.json();
    if (result.ok) {
      status.style.color = 'green';
      status.textContent = 'Logged in — redirecting…';
      window.location.href = '/fitness';
    } else {
      status.textContent = result.error || 'Login failed';
      document.getElementById('pin-pin').value = '';
    }
  } catch (err) {
    status.textContent = err.message || 'Login failed';
  } finally {
    btn.disabled = false;
  }
  return false;
}
