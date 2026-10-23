(handler as Function).apply(null, args);
(handler as any).method.apply(handler as any, args);
handler!.method.apply(handler!, args);
handler.apply<string>(null, args);
