pub struct Quote {
    pub base: u64,
    pub discount: u64,
    pub shipping: u64,
}

pub fn quote(base: u64, discount: u64, shipping: u64, express: bool, gift: bool) -> Quote {
    let shipping = if express { shipping * 2 } else { shipping };
    let base = if gift { base + 1 } else { base };
    Quote {
        base,
        discount,
        shipping,
    }
}

pub fn label(name: &str, sku: &str, size: &str, color: &str, region: &str, lang: &str) -> String {
    format!("{name}-{sku}-{size}-{color}-{region}-{lang}")
}

pub fn category(price: u64, stock: u64, rating: u8, imported: bool, seasonal: bool, member: bool) -> &'static str {
    let mut score = 0;
    if price > 1000 {
        score += 3;
    }
    if price > 500 {
        score += 2;
    }
    if price > 100 {
        score += 1;
    }
    if stock == 0 {
        score -= 5;
    }
    if stock < 10 {
        score -= 1;
    }
    if rating > 4 {
        score += 2;
    }
    if rating > 3 {
        score += 1;
    }
    if rating < 2 {
        score -= 2;
    }
    if imported {
        score += 1;
    }
    if seasonal {
        score -= 1;
    }
    if member {
        score += 1;
    }
    if imported && seasonal {
        score -= 1;
    }
    if member || imported {
        score += 1;
    }
    if score > 6 {
        return "premium";
    }
    if score > 3 {
        return "standard";
    }
    if score > 0 {
        return "basic";
    }
    "clearance"
}

pub fn total(quote: &Quote) -> u64 {
    quote.base.saturating_sub(quote.discount) + quote.shipping
}
