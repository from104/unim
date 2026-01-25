# Development Conventions

* The core logic is isolated in the root `src/` directory. Any changes to the fundamental input logic should be made there.
* The GNOME extension communicates with the Rust engine by executing the `unim-cli` binary as a subprocess. This provides a stable and sandbox-friendly integration.
* The `Makefile` is the source of truth for the standard build and installation process.
* **문서 작성 언어**: Walkthrough, 계획(Implementation Plan), 작업 목록(Task) 등 문서는 **한글로 작성**합니다.
