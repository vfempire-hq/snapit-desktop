# Azure Trusted Signing — VF Empire onboarding

Kills SmartScreen "unrecognised app" warnings on Windows installers for
every VF product — signed under a single verified subject
("VF Empire Corp Ltd"). ~$10/month, no hardware token, Microsoft directly
issues the certs, integrates natively with GitHub Actions.

Once VF Empire is onboarded as a verified subject:
- SnapIT installers ship signed and SmartScreen-clean
- VF Mail installers same
- Every future VF desktop app inherits the same signing pipeline

---

## Vincent's steps (need Azure sign-in + payment method)

**All of this needs to be done once from an Azure Portal session using
an account tied to VF Empire Corp Ltd's official email
(security@vfempire.com is the right one — it's the Companies House
contact record).**

### 1 · Sign in to Azure Portal

Go to https://portal.azure.com/ and sign in with your Microsoft account.
If VF Empire doesn't yet have an Azure tenant, one gets created here.

### 2 · Add a payment method

Billing → Payment methods → Add. Corporate credit card OK. Estimate is
$10-15/month (Trusted Signing is ~$10 + per-signature at fractions of a
cent — negligible for our volume).

### 3 · Register the Trusted Signing resource

- Portal search bar: "Trusted Signing"
- Click **Create Trusted Signing account**
- Resource group: create `vf-signing` (or reuse existing)
- Region: **West Europe** (Malta-closest CA endpoint)
- Account name: `vf-empire-signing`
- Pricing tier: **Standard** ($9.99/month base)
- Click **Review + create** → **Create**

### 4 · Create an identity validation

Inside the new Trusted Signing account:
- **Identity validations** → **Add**
- Type: **Public**
- Subject / display name: `VF Empire Corp Ltd`
- Address: your Malta registered office
- Country: Malta
- Companies House / equivalent registration number: (VF Empire's Malta MFSA number)
- Contact: security@vfempire.com

Submit. Microsoft's compliance team reviews this — typically 24-72 hours.
You get an email when it's approved. **This is the one-time verification
that unlocks zero-warning signing.**

### 5 · Create a certificate profile

Once identity validation is approved:
- Trusted Signing account → **Certificate profiles** → **Create**
- Profile name: `vf-empire-public`
- Certificate type: **Public trust**
- Identity validation: (select the one just approved)
- Include timestamp: **Yes**

### 6 · Get the info I need to wire the CI

Send me these three values (they're safe to share — they're identifiers, not secrets):

- **Trusted Signing account endpoint** (looks like `https://weu.codesigning.azure.net/`)
- **Account name** (`vf-empire-signing`)
- **Certificate profile name** (`vf-empire-public`)

### 7 · Create a service principal for GitHub Actions

Portal → **Microsoft Entra ID** → **App registrations** → **New registration**
- Name: `vf-github-signing`
- Supported account types: Single tenant

Note the **Application (client) ID** and **Directory (tenant) ID**.
Then:
- App → **Certificates & secrets** → **New client secret**
- Description: `github-actions-signing`
- Expires: 24 months
- **Copy the secret value now** (never shown again)

Give the service principal signing access:
- Trusted Signing account → **Access control (IAM)** → **Add role assignment**
- Role: **Trusted Signing Certificate Profile Signer**
- Assign to: the `vf-github-signing` app

Send me these three values as GitHub Secrets — I'll wire them into the
signing step:
- `AZURE_TENANT_ID`
- `AZURE_CLIENT_ID`
- `AZURE_CLIENT_SECRET`

---

## What I do once Vincent hands over the values

I'll add this step to `.github/workflows/release.yml` for the Windows
build leg (SnapIT + all future VF apps get the same pattern):

```yaml
- name: Sign Windows installers with Azure Trusted Signing
  if: matrix.os == 'windows-latest'
  uses: azure/trusted-signing-action@v0.5
  with:
    azure-tenant-id:      ${{ secrets.AZURE_TENANT_ID }}
    azure-client-id:      ${{ secrets.AZURE_CLIENT_ID }}
    azure-client-secret:  ${{ secrets.AZURE_CLIENT_SECRET }}
    endpoint:             https://weu.codesigning.azure.net/
    trusted-signing-account-name: vf-empire-signing
    certificate-profile-name:     vf-empire-public
    files-folder:         app/src-tauri/target/x86_64-pc-windows-msvc/release/bundle
    files-folder-filter:  exe,msi
    file-digest:          SHA256
    timestamp-rfc3161:    http://timestamp.acs.microsoft.com
    timestamp-digest:     SHA256
```

Once merged and the next tag is pushed:
- SnapIT installer downloads from snapit.vfempire.com/downloads/win-setup **no SmartScreen warning**
- VF Mail installers same
- Signature says "Publisher: VF Empire Corp Ltd" verified

---

## The two things Cloudflare does NOT provide

Vincent asked whether the Cloudflare-hosted download URL is enough.
Short answer: **no**, because Cloudflare gives us TLS/HTTPS certs, not
Authenticode code-signing certs.

- **HTTPS cert (Cloudflare, free)** — encrypts bytes between our edge and the
  browser. Padlock icon. Proves nobody tampered with the download in transit.
- **Code-signing cert (Azure Trusted Signing, $10/mo)** — embedded inside
  the .exe file itself. Proves the file was built and released by
  "VF Empire Corp Ltd" — a company Microsoft has verified through Malta
  MFSA records. This is what SmartScreen actually reads.

We need both. Cloudflare gives us the courier; Azure gives us the wax seal.
