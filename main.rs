use chrono::{offset::TimeZone, DateTime, Local, NaiveDateTime};
use reqwest::{
    self,
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;

#[derive(Serialize, Deserialize, Debug)]
struct Product {
    id: String,
    object: String,
    active: bool,
    created: i32,
    default_price: Option<String>,
    description: Option<String>,
    images: Vec<String>,
    livemode: bool,
    metadata: HashMap<String, String>,
    name: String,
    package_dimensions: Option<String>,
    shippable: Option<String>,
    statement_descriptor: Option<String>,
    tax_code: Option<String>,
    unit_label: Option<String>,
    updated: i32,
    url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ProductsResponse {
    object: String,
    url: String,
    has_more: bool,
    data: Vec<Product>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Coupon {
    id: String,
    object: String,
    amount_off: Option<i32>,
    created: i32,
    currency: Option<String>,
    duration: String,
    duration_in_months: Option<i32>,
    livemode: bool,
    max_redemptions: Option<i32>,
    metadata: HashMap<String, String>,
    name: Option<String>,
    percent_off: f32,
    redeem_by: Option<i32>,
    times_redeemed: i32,
    valid: bool,
}

#[derive(Serialize, Deserialize, Debug)]
struct CouponRequest {
    name: String,
    percent_off: f32,
    redeem_by: i64,
    applies_to: CouponAppliesTo,
}

#[derive(Serialize, Deserialize, Debug)]
struct CouponAppliesTo {
    products: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct PromotionCodeRequest {
    coupon: String,
    code: String,
    expires_at: i64,
    max_redemptions: i32,
    restrictions: PromotionCodeRestrictions,
}

#[derive(Serialize, Deserialize, Debug)]
struct PromotionCodeRestrictions {
    first_time_transaction: bool,
}

fn generate_random_code() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890";
    const STR_LEN: usize = 6;
    let mut rng = rand::thread_rng();

    let rand_str: String = (0..STR_LEN)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();
    return rand_str;
}

#[tokio::main]
async fn main() {
    println!("[ Rusty Voucher ]");

    let first_time_transaction = false;

    let mut stripe_key = String::new();
    println!("Enter your Stripe key:");
    std::io::stdin().read_line(&mut stripe_key).unwrap();
    stripe_key = stripe_key.trim().to_owned();

    // Input coupon name
    let mut coupon_name = String::new();
    println!("Coupon name: ");
    std::io::stdin().read_line(&mut coupon_name).unwrap();

    // Input expiration date
    let mut expiration_string = String::new();
    println!("Expiration date (YYYY-MM-DD):");
    std::io::stdin().read_line(&mut expiration_string).unwrap();
    let parse_result = NaiveDateTime::parse_from_str(
        &format!("{} 23:59:59", &expiration_string.trim()),
        "%Y-%m-%d %H:%M:%S",
    );
    let expiration_date: DateTime<Local> = match parse_result {
        Ok(date) => Local.from_local_datetime(&date).unwrap(),
        Err(_) => {
            println!("Cannot parse date. Aborting.");
            return;
        }
    };

    let mut requested_code_count_string = String::new();
    println!("How many codes do you need:");
    std::io::stdin()
        .read_line(&mut requested_code_count_string)
        .unwrap();
    let parsed_requested_code_count = requested_code_count_string.trim().parse::<i32>();
    let requested_code_count = match parsed_requested_code_count {
        Ok(result) => result,
        Err(_) => {
            println!("Cannot parse number of vouchers. Aborting.");
            return;
        }
    };

    if requested_code_count <= 0 {
        println!("Number of codes must be more than zero.");
        return;
    }

    let client = reqwest::Client::new();
    let product_response = client
        .get("https://api.stripe.com/v1/products")
        .header(AUTHORIZATION, format!("Bearer {}", stripe_key))
        .header(ACCEPT, "application/json")
        .send()
        .await
        .unwrap();

    if product_response.status() == reqwest::StatusCode::UNAUTHORIZED {
        println!("Unauthorized: Probably wrong stripe key");
        return;
    }

    if product_response.status() != reqwest::StatusCode::OK {
        println!("Unexpected error");
        return;
    }

    let data: ProductsResponse = product_response.json().await.unwrap();

    println!("Select a product from list:");

    let mut products: Vec<String> = Vec::new();
    let mut i = 0;

    for item in data.data {
        products.push(item.id.clone());
        println!("[{}] {}", i, item.name);
        i += 1;
    }

    if products.is_empty() {
        println!("No available products");
        return;
    }

    let mut requested_product_id_string = String::new();
    std::io::stdin()
        .read_line(&mut requested_product_id_string)
        .unwrap();
    let parsed_requested_product_id = requested_product_id_string.trim().parse::<usize>();
    let requested_product_id = match parsed_requested_product_id {
        Ok(result) => result,
        Err(_) => {
            println!("Cannot parse selected ID of product. Aborting.");
            return;
        }
    };

    if requested_product_id > products.len() {
        println!("Invalid product selected");
        return;
    }

    print!("Creating a coupon...");

    let coupon_request = CouponRequest {
        name: coupon_name.trim().to_owned(),
        percent_off: 100.0,
        redeem_by: expiration_date.timestamp(),
        applies_to: CouponAppliesTo {
            products: vec![products[requested_product_id].clone()],
        },
    };

    let coupon_request_body = serde_qs::to_string(&coupon_request).unwrap();

    let coupon_response = client
        .post("https://api.stripe.com/v1/coupons")
        .header(AUTHORIZATION, format!("Bearer {}", stripe_key))
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(coupon_request_body)
        .send()
        .await
        .unwrap();

    if coupon_response.status() == reqwest::StatusCode::UNAUTHORIZED {
        println!("Unauthorized: Probably wrong stripe key");
        return;
    }

    if coupon_response.status() == reqwest::StatusCode::BAD_REQUEST {
        println!("Coupon cannot be created");
        return;
    }

    if coupon_response.status() != reqwest::StatusCode::OK {
        println!("Unexpected error");
        return;
    }

    let coupon: Coupon = coupon_response.json().await.unwrap();

    println!("[ DONE ]");

    //
    // Create promotion codes
    //
    let mut created_code_count = 0;

    let mut file = File::create("vouchers.txt").unwrap();

    while created_code_count < requested_code_count {
        let mut restrictions = HashMap::new();
        restrictions.insert(String::from("first_time_transaction"), String::from("true"));

        let promotion_code_request = PromotionCodeRequest {
            coupon: coupon.id.clone(),
            code: generate_random_code(),
            expires_at: expiration_date.timestamp(),
            max_redemptions: 1,
            restrictions: PromotionCodeRestrictions {
                first_time_transaction,
            },
        };

        let promotion_code_request_body = serde_qs::to_string(&promotion_code_request).unwrap();

        let promotion_code_response = client
            .post("https://api.stripe.com/v1/promotion_codes")
            .header(AUTHORIZATION, format!("Bearer {}", stripe_key))
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(promotion_code_request_body)
            .send()
            .await
            .unwrap();

        if promotion_code_response.status() == reqwest::StatusCode::UNAUTHORIZED {
            println!("Unauthorized: Probably wrong stripe key");
            break;
        }

        if promotion_code_response.status() == reqwest::StatusCode::BAD_REQUEST {
            continue;
        }

        if promotion_code_response.status() != reqwest::StatusCode::OK {
            println!("Unexpected error");
            break;
        }

        created_code_count += 1;
        println!(
            "Promotion code {} or {} [{}]",
            created_code_count, requested_code_count, promotion_code_request.code
        );
        writeln!(&mut file, "{}", promotion_code_request.code).unwrap();
    }
}
