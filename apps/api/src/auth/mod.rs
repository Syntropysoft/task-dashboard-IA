//! Identidad: syntroAuth autentica, esta app autoriza. Acá solo se decide *quién es* — a partir
//! de un JWT RS256 de syntroAuth (`/api/*`) — nunca *qué puede*.

pub mod extract;
pub mod jwt;

pub use extract::AuthUser;
pub use jwt::{AuthError, Claims, JwtConfig, Validator};
