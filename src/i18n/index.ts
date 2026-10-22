import { useCallback } from "react";
import { useApp } from "../store/app";
import type { Language } from "../lib/bindings/Language";
import { en } from "./en";
import { ptBR, type MessageKey, type Messages } from "./pt-BR";

export type { MessageKey };
export type Locale = "pt-BR" | "en";

const catalogs: Record<Locale, Messages> = { "pt-BR": ptBR, en };

export function resolveLocale(language: Language, navigatorLanguage = globalThis.navigator?.language ?? "pt-BR"): Locale {
  if (language === "pt-BR" || language === "en") return language;
  return navigatorLanguage.toLowerCase().startsWith("pt") ? "pt-BR" : "en";
}

export type Params = Record<string, string | number>;

export function translate(locale: Locale, key: MessageKey, params?: Params): string {
  const template = catalogs[locale][key] ?? ptBR[key] ?? key;
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (match, name: string) =>
    name in params ? String(params[name]) : match,
  );
}

/** Picks `${key}One` for a count of one when such a key exists. */
export function plural(key: MessageKey, count: number): MessageKey {
  const one = `${key}One` as MessageKey;
  return count === 1 && one in ptBR ? one : key;
}

export type Translator = ((key: MessageKey, params?: Params) => string) & { locale: Locale };

export function useT(): Translator {
  const language = useApp((s) => s.settings.language);
  const locale = resolveLocale(language);
  const t = useCallback((key: MessageKey, params?: Params) => translate(locale, key, params), [locale]);
  return Object.assign(t, { locale });
}
