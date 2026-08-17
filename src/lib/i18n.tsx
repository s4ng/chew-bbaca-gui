import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";

import { en } from "./messages/en";
import { ko, type Messages } from "./messages/ko";

/**
 * 화면 문자열의 언어 전환 (v0.4.3~).
 *
 * ## 왜 라이브러리를 쓰지 않는가
 *
 * 번역 단위가 300개 남짓이고 복수형·성별 같은 규칙이 필요한 문장이 없다. i18next
 * 를 끌어오면 런타임 템플릿 파서 위에 문자열 키를 얹게 되는데, 그러면 **키 오타가
 * 런타임에야 드러난다**. 대신 카탈로그를 그냥 객체로 두고 값이 필요한 문장은
 * 함수로 적는다 — `en` 이 `typeof ko` 를 따르므로 키가 빠지거나 인자 개수가
 * 어긋나면 `tsc` 가 잡는다.
 *
 * ## 한국어가 원본이다
 *
 * `ko.ts` 가 타입을 정의하고 `en.ts` 가 그것을 구현한다. 새 문구는 반드시 한국어
 * 쪽에 먼저 쓰고, 영어를 안 채우면 빌드가 깨진다. 번역이 조용히 뒤처지는 것을
 * 막는 장치이므로 `Partial<Messages>` 로 느슨하게 만들지 마라.
 *
 * ## 여기까지가 범위다
 *
 * **Rust 가 만든 문자열은 한국어 그대로 나온다.** 에러 메시지는 `jobs.rs`·`wsl.rs`
 * 의 워커 스레드 깊은 곳에서 만들어져 로케일을 알 방법이 없고, 그것을 고치려면
 * `Error` 를 `{kind, args}` 계약으로 바꿔 문장 조립을 프런트로 옮겨야 한다.
 * 그 작업은 별도이고, `Error::kind` 가 이미 안정 식별자라 발판은 마련돼 있다.
 */
export type Lang = "ko" | "en";

/** 사용자가 고르는 값. `auto` 는 OS 표시 언어를 따른다. */
export type LangSetting = "auto" | Lang;

const CATALOGS: Record<Lang, Messages> = { ko, en };

/**
 * 설정 저장소로 `localStorage` 를 쓴다.
 *
 * `app.db` 가 아닌 이유는 두 가지다. 첫째, 이 값은 **온보딩 화면이 그려지기 전에**
 * 필요한데 `settingsGet()` 은 비동기라 첫 프레임이 반대 언어로 한 번 깜빡인다.
 * 둘째, 백엔드 문자열은 어차피 이 설정을 보지 않으므로 순수한 표현 계층 설정이다.
 */
const STORAGE_KEY = "chewie.lang";

/**
 * OS 표시 언어. WebView2 의 `navigator.language` 는 Windows 표시 언어를 따라간다.
 *
 * `@tauri-apps/plugin-os` 의 `locale()` 을 쓰지 않는 이유: 플러그인과 권한을 하나
 * 더 붙여야 하는데 얻는 것이 같다.
 */
export function detectLang(): Lang {
  const tags = [navigator.language, ...(navigator.languages ?? [])];
  return tags.some((t) => t?.toLowerCase().startsWith("ko")) ? "ko" : "en";
}

export function resolveLang(setting: LangSetting): Lang {
  return setting === "auto" ? detectLang() : setting;
}

function readSetting(): LangSetting {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    if (v === "ko" || v === "en" || v === "auto") return v;
  } catch {
    // 웹뷰가 스토리지를 막은 경우. 자동 판정으로 떨어지면 그만이다.
  }
  return "auto";
}

interface I18n {
  /** 현재 카탈로그. 컴포넌트는 `t.settings.title` 처럼 바로 읽는다. */
  t: Messages;
  lang: Lang;
  setting: LangSetting;
  setSetting: (next: LangSetting) => void;
}

const I18nContext = createContext<I18n | null>(null);

export function I18nProvider({ children }: { children: React.ReactNode }) {
  const [setting, setStored] = useState<LangSetting>(readSetting);
  const lang = resolveLang(setting);

  const setSetting = useCallback((next: LangSetting) => {
    setStored(next);
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // 저장에 실패해도 이번 세션에는 적용된다. 막을 이유가 없다.
    }
  }, []);

  // 문서 언어를 맞춘다. 웹뷰의 줄바꿈 규칙과 접근성 도구가 이 값을 본다.
  useEffect(() => {
    document.documentElement.lang = lang;
  }, [lang]);

  const value = useMemo<I18n>(
    () => ({ t: CATALOGS[lang], lang, setting, setSetting }),
    [lang, setting, setSetting],
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

function useI18n(): I18n {
  const ctx = useContext(I18nContext);
  if (!ctx) throw new Error("I18nProvider 밖에서 번역을 쓸 수 없습니다");
  return ctx;
}

/** 화면 문자열. 가장 많이 쓰는 훅이라 이름을 짧게 둔다. */
export function useT(): Messages {
  return useI18n().t;
}

/** 언어 선택 UI 전용. 나머지 화면은 `useT()` 만 있으면 된다. */
export function useLangSetting(): Pick<I18n, "lang" | "setting" | "setSetting"> {
  const { lang, setting, setSetting } = useI18n();
  return { lang, setting, setSetting };
}
