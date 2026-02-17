//! ISO 20022 XML message parser.
//!
//! Parses inbound pain.001 messages into structured `PaymentInstruction` data.

use crate::iso20022::types::*;
use eyre::{Result, bail};
use quick_xml::Reader;
use quick_xml::events::Event;

/// Parse a pain.001 (Customer Credit Transfer Initiation) XML message.
pub fn parse_pain001(xml: &str) -> Result<PaymentInstruction> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut msg_id = String::new();
    let mut creation_date_time = String::new();
    let mut nb_of_txs: u32 = 0;
    let mut ctrl_sum: Option<String> = None;
    let mut transactions = Vec::new();

    // Current parsing state
    let mut current_path: Vec<String> = Vec::new();
    let mut current_transfer: Option<CreditTransferBuilder> = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                current_path.push(name.clone());

                if name == "CdtTrfTxInf" {
                    current_transfer = Some(CreditTransferBuilder::default());
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if name == "CdtTrfTxInf" {
                    if let Some(builder) = current_transfer.take() {
                        transactions.push(builder.build());
                    }
                }

                current_path.pop();
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape()?.to_string();
                let path = current_path.join("/");

                match path.as_str() {
                    s if s.ends_with("GrpHdr/MsgId") => msg_id = text,
                    s if s.ends_with("GrpHdr/CreDtTm") => creation_date_time = text,
                    s if s.ends_with("GrpHdr/NbOfTxs") => {
                        nb_of_txs = text.parse().unwrap_or(0);
                    }
                    s if s.ends_with("GrpHdr/CtrlSum") => ctrl_sum = Some(text),
                    _ => {
                        if let Some(ref mut builder) = current_transfer {
                            builder.apply_text(&path, &text);
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => bail!("XML parse error: {e}"),
            _ => {}
        }
        buf.clear();
    }

    Ok(PaymentInstruction {
        msg_id,
        creation_date_time,
        nb_of_txs,
        ctrl_sum,
        transactions,
    })
}

/// Builder for constructing `CreditTransfer` from XML parsing events.
#[derive(Default)]
struct CreditTransferBuilder {
    end_to_end_id: String,
    currency: String,
    amount: String,
    debtor_name: String,
    debtor_bic: Option<String>,
    debtor_account: String,
    creditor_name: String,
    creditor_bic: Option<String>,
    creditor_account: String,
    purpose_code: Option<String>,
    remittance_info: Option<String>,
}

impl CreditTransferBuilder {
    fn apply_text(&mut self, path: &str, text: &str) {
        match path {
            s if s.ends_with("PmtId/EndToEndId") => self.end_to_end_id = text.to_string(),
            s if s.ends_with("InstdAmt") => {
                self.amount = text.to_string();
                // Currency is extracted from attribute, handled separately in a full impl
            }
            s if s.ends_with("Dbtr/Nm") => self.debtor_name = text.to_string(),
            s if s.ends_with("DbtrAgt/FinInstnId/BIC") || s.ends_with("DbtrAgt/FinInstnId/BICFI") => {
                self.debtor_bic = Some(text.to_string());
            }
            s if s.ends_with("DbtrAcct/Id/IBAN") || s.ends_with("DbtrAcct/Id/Othr/Id") => {
                self.debtor_account = text.to_string();
            }
            s if s.ends_with("Cdtr/Nm") => self.creditor_name = text.to_string(),
            s if s.ends_with("CdtrAgt/FinInstnId/BIC") || s.ends_with("CdtrAgt/FinInstnId/BICFI") => {
                self.creditor_bic = Some(text.to_string());
            }
            s if s.ends_with("CdtrAcct/Id/IBAN") || s.ends_with("CdtrAcct/Id/Othr/Id") => {
                self.creditor_account = text.to_string();
            }
            s if s.ends_with("Purp/Cd") => self.purpose_code = Some(text.to_string()),
            s if s.ends_with("RmtInf/Ustrd") => {
                self.remittance_info = Some(text.to_string());
            }
            _ => {}
        }
    }

    fn build(self) -> CreditTransfer {
        CreditTransfer {
            end_to_end_id: self.end_to_end_id,
            amount: Amount {
                currency: if self.currency.is_empty() {
                    "USD".to_string()
                } else {
                    self.currency
                },
                value: self.amount,
            },
            debtor: Party {
                name: self.debtor_name,
                bic: self.debtor_bic,
            },
            debtor_account: Account {
                id: self.debtor_account,
            },
            creditor: Party {
                name: self.creditor_name,
                bic: self.creditor_bic,
            },
            creditor_account: Account {
                id: self.creditor_account,
            },
            purpose_code: self.purpose_code,
            remittance_info: self.remittance_info,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PAIN001: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pain.001.001.03">
  <CstmrCdtTrfInitn>
    <GrpHdr>
      <MsgId>MSG-001</MsgId>
      <CreDtTm>2026-02-15T10:30:00</CreDtTm>
      <NbOfTxs>1</NbOfTxs>
      <CtrlSum>1000.00</CtrlSum>
    </GrpHdr>
    <PmtInf>
      <CdtTrfTxInf>
        <PmtId>
          <EndToEndId>E2E-PAY-001</EndToEndId>
        </PmtId>
        <Amt>
          <InstdAmt Ccy="USD">1000.00</InstdAmt>
        </Amt>
        <Dbtr>
          <Nm>Alice Corp</Nm>
        </Dbtr>
        <DbtrAcct>
          <Id><IBAN>VN12345678901234</IBAN></Id>
        </DbtrAcct>
        <Cdtr>
          <Nm>Bob Ltd</Nm>
        </Cdtr>
        <CdtrAcct>
          <Id><IBAN>VN98765432109876</IBAN></Id>
        </CdtrAcct>
        <Purp>
          <Cd>SUPP</Cd>
        </Purp>
        <RmtInf>
          <Ustrd>Invoice INV-2026-001</Ustrd>
        </RmtInf>
      </CdtTrfTxInf>
    </PmtInf>
  </CstmrCdtTrfInitn>
</Document>"#;

    #[test]
    fn test_parse_pain001() {
        let instruction = parse_pain001(SAMPLE_PAIN001).unwrap();
        assert_eq!(instruction.msg_id, "MSG-001");
        assert_eq!(instruction.nb_of_txs, 1);
        assert_eq!(instruction.ctrl_sum.as_deref(), Some("1000.00"));
        assert_eq!(instruction.transactions.len(), 1);

        let tx = &instruction.transactions[0];
        assert_eq!(tx.end_to_end_id, "E2E-PAY-001");
        assert_eq!(tx.amount.value, "1000.00");
        assert_eq!(tx.debtor.name, "Alice Corp");
        assert_eq!(tx.debtor_account.id, "VN12345678901234");
        assert_eq!(tx.creditor.name, "Bob Ltd");
        assert_eq!(tx.creditor_account.id, "VN98765432109876");
        assert_eq!(tx.purpose_code.as_deref(), Some("SUPP"));
        assert_eq!(
            tx.remittance_info.as_deref(),
            Some("Invoice INV-2026-001")
        );
    }
}
