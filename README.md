# Daymark · Rust 체크리스트 개발

바탕화면에 띄우는 달력 스티커형 체크리스트 프로젝트입니다.

## 폴더

- `frontend/` — 동작하는 HTML, CSS, JavaScript 화면
- `rust-practice/src-tauri/` — Windows 위젯과 설치 프로그램
- `docs/mockup.html` — 디자인 목업
- `docs/flowchart.md` — 사용 흐름도와 구현 경계

## Windows에서 실행과 설치 파일 빌드

프로젝트 최상위 폴더의 PowerShell에서 실행합니다.

```powershell
cd rust-practice
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
cargo tauri dev
```

같은 폴더에서 아래 명령으로 Windows 설치 파일을 다시 빌드할 수 있습니다.

```powershell
cargo tauri build --bundles nsis -- --locked
```

설치 파일은 `rust-practice/src-tauri/target/release/bundle/nsis/`에 생성됩니다. 설치하면 현재 사용자 계정에 Daymark가 등록되어 Windows 로그인 시 실행됩니다. Windows의 시작프로그램 설정에서 끌 수 있고, Daymark를 제거하면 등록도 해제됩니다.

할 일, 메모, 창 배치는 `%LOCALAPPDATA%\io.daymark.desktop`에 저장됩니다. 이전 데스크톱 버전이 `%LOCALAPPDATA%\com.tauri.dev`에 저장한 파일은 첫 실행 시 복사됩니다.

공개 배포용으로 서명할 때는 코드 서명 인증서를 Windows의 현재 사용자 개인 인증서 저장소에 준비한 뒤 `rust-practice`에서 실행합니다. `-TimestampUrl`에는 인증서 발급사가 안내하는 RFC 3161 주소를 지정합니다.

```powershell
.\build-signed.ps1 -TimestampUrl '<인증서 발급사의 RFC 3161 주소>'
```

인증서가 여러 개인 경우 `-CertificateThumbprint`로 하나를 지정합니다. 스크립트는 앱과 설치 파일의 게시자 서명 및 타임스탬프를 확인합니다.

## GitHub Releases 업데이트

GitHub 저장소 `DJU-ADAS-LAB/Daymark`의 Settings → Secrets and variables → Actions에 `TAURI_SIGNING_PRIVATE_KEY`를 등록합니다. 값은 이 PC의 `C:\Users\COMPUTER1\.daymark-signing\updater.key` 파일 내용입니다. 암호를 설정한 키라면 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`도 등록합니다. 개인 키는 Git에 올리지 말고 별도로 백업합니다.

`tauri.conf.json`과 `Cargo.toml`의 버전을 함께 올리고 `Cargo.lock`을 갱신한 뒤, 소스를 커밋하고 해당 버전 태그(예: `v0.1.3`)를 푸시하면 GitHub Actions가 Windows 설치 파일, 서명, `latest.json`을 포함한 릴리스 초안을 만듭니다. 초안을 게시하면 설치된 Daymark가 다음 시작 시 업데이트를 확인합니다. 달력을 우클릭해 수동 확인도 가능합니다.

기존 0.1.1/0.1.2 설치 파일에는 업데이트 기능이 없으므로, 업데이트 기능을 포함한 설치 파일을 처음 한 번 직접 설치해야 합니다. 공개 저장소의 Releases를 사용하는 구성입니다. 비공개 저장소 배포에는 별도의 인증 가능한 업데이트 배포 서버가 필요합니다.

로컬 빌드도 업데이트 서명이 필요합니다.

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = "$env:USERPROFILE\.daymark-signing\updater.key"
cargo tauri build --bundles nsis -- --locked
```

이 업데이트 검증용 서명은 Windows 게시자 코드 서명과 별개입니다.
