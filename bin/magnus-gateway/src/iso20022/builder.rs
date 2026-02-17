//! ISO 20022 XML message builder.
//!
//! Builds outbound XML messages: pacs.008, camt.053, camt.054.

use crate::iso20022::types::*;

/// Build a pacs.008 (FI-to-FI Customer Credit Transfer) XML message.
pub fn build_pacs008(settlement: &SettlementData) -> String {
    let tx = &settlement.instruction;
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.008.001.02">
  <FIToFICstmrCdtTrf>
    <GrpHdr>
      <MsgId>{msg_id}</MsgId>
      <CreDtTm>{settlement_time}</CreDtTm>
      <NbOfTxs>1</NbOfTxs>
      <SttlmInf>
        <SttlmMtd>CLRG</SttlmMtd>
      </SttlmInf>
      <InstgAgt>
        <FinInstnId><BICFI>{instructing_agent}</BICFI></FinInstnId>
      </InstgAgt>
      <InstdAgt>
        <FinInstnId><BICFI>{instructed_agent}</BICFI></FinInstnId>
      </InstdAgt>
    </GrpHdr>
    <CdtTrfTxInf>
      <PmtId>
        <EndToEndId>{end_to_end_id}</EndToEndId>
        <TxId>{chain_tx_hash}</TxId>
      </PmtId>
      <IntrBkSttlmAmt Ccy="{currency}">{amount}</IntrBkSttlmAmt>
      <IntrBkSttlmDt>{settlement_date}</IntrBkSttlmDt>
      <Dbtr>
        <Nm>{debtor_name}</Nm>
      </Dbtr>
      <DbtrAcct>
        <Id><Othr><Id>{debtor_account}</Id></Othr></Id>
      </DbtrAcct>
      <Cdtr>
        <Nm>{creditor_name}</Nm>
      </Cdtr>
      <CdtrAcct>
        <Id><Othr><Id>{creditor_account}</Id></Othr></Id>
      </CdtrAcct>{purpose_block}{remittance_block}
    </CdtTrfTxInf>
  </FIToFICstmrCdtTrf>
</Document>"#,
        msg_id = format!("PACS008-{}", tx.end_to_end_id),
        settlement_time = settlement.settlement_time,
        instructing_agent = settlement.instructing_agent,
        instructed_agent = settlement.instructed_agent,
        end_to_end_id = tx.end_to_end_id,
        chain_tx_hash = settlement.chain_tx_hash,
        currency = tx.amount.currency,
        amount = tx.amount.value,
        settlement_date = &settlement.settlement_time[..10], // YYYY-MM-DD
        debtor_name = tx.debtor.name,
        debtor_account = tx.debtor_account.id,
        creditor_name = tx.creditor.name,
        creditor_account = tx.creditor_account.id,
        purpose_block = tx.purpose_code.as_ref().map_or(String::new(), |code| {
            format!("\n      <Purp><Cd>{code}</Cd></Purp>")
        }),
        remittance_block = tx.remittance_info.as_ref().map_or(String::new(), |info| {
            format!("\n      <RmtInf><Ustrd>{info}</Ustrd></RmtInf>")
        }),
    )
}

/// Build a camt.053 (Bank-to-Customer Statement) XML message.
pub fn build_camt053(
    statement_id: &str,
    account_id: &str,
    entries: &[StatementEntry],
    creation_time: &str,
) -> String {
    let entries_xml: String = entries
        .iter()
        .map(|entry| {
            format!(
                r#"
      <Ntry>
        <NtryRef>{entry_ref}</NtryRef>
        <Amt Ccy="{currency}">{amount}</Amt>
        <CdtDbtInd>{cd_indicator}</CdtDbtInd>
        <BookgDt><Dt>{booking_date}</Dt></BookgDt>
        <ValDt><Dt>{value_date}</Dt></ValDt>
        <NtryDtls>
          <TxDtls>
            <Refs><EndToEndId>{end_to_end_id}</EndToEndId></Refs>{remittance}
          </TxDtls>
        </NtryDtls>
      </Ntry>"#,
                entry_ref = entry.entry_ref,
                currency = entry.amount.currency,
                amount = entry.amount.value,
                cd_indicator = entry.cd_indicator,
                booking_date = entry.booking_date,
                value_date = entry.value_date,
                end_to_end_id = entry.end_to_end_id,
                remittance = entry.remittance_info.as_ref().map_or(String::new(), |info| {
                    format!("\n            <RmtInf><Ustrd>{info}</Ustrd></RmtInf>")
                }),
            )
        })
        .collect();

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.053.001.02">
  <BkToCstmrStmt>
    <GrpHdr>
      <MsgId>{statement_id}</MsgId>
      <CreDtTm>{creation_time}</CreDtTm>
    </GrpHdr>
    <Stmt>
      <Id>{statement_id}</Id>
      <Acct>
        <Id><Othr><Id>{account_id}</Id></Othr></Id>
      </Acct>{entries_xml}
    </Stmt>
  </BkToCstmrStmt>
</Document>"#,
    )
}

/// Build a camt.054 (Bank-to-Customer Debit/Credit Notification) XML message.
pub fn build_camt054(notification: &NotificationEntry, creation_time: &str) -> String {
    let entry = &notification.entry;
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.054.001.02">
  <BkToCstmrDbtCdtNtfctn>
    <GrpHdr>
      <MsgId>NTFCTN-{ntfctn_id}</MsgId>
      <CreDtTm>{creation_time}</CreDtTm>
    </GrpHdr>
    <Ntfctn>
      <Id>{ntfctn_id}</Id>
      <Acct>
        <Id><Othr><Id>{account_id}</Id></Othr></Id>
      </Acct>
      <Ntry>
        <NtryRef>{entry_ref}</NtryRef>
        <Amt Ccy="{currency}">{amount}</Amt>
        <CdtDbtInd>{cd_indicator}</CdtDbtInd>
        <BookgDt><Dt>{booking_date}</Dt></BookgDt>
        <ValDt><Dt>{value_date}</Dt></ValDt>
        <NtryDtls>
          <TxDtls>
            <Refs><EndToEndId>{end_to_end_id}</EndToEndId></Refs>
          </TxDtls>
        </NtryDtls>
      </Ntry>
    </Ntfctn>
  </BkToCstmrDbtCdtNtfctn>
</Document>"#,
        ntfctn_id = notification.ntfctn_id,
        creation_time = creation_time,
        account_id = notification.account_id,
        entry_ref = entry.entry_ref,
        currency = entry.amount.currency,
        amount = entry.amount.value,
        cd_indicator = entry.cd_indicator,
        booking_date = entry.booking_date,
        value_date = entry.value_date,
        end_to_end_id = entry.end_to_end_id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::iso20022::parser;

    #[test]
    fn test_build_pacs008_roundtrip() {
        let settlement = SettlementData {
            instruction: CreditTransfer {
                end_to_end_id: "E2E-001".to_string(),
                amount: Amount {
                    currency: "USD".to_string(),
                    value: "500.00".to_string(),
                },
                debtor: Party {
                    name: "Alice".to_string(),
                    bic: None,
                },
                debtor_account: Account {
                    id: "0xAliceAddr".to_string(),
                },
                creditor: Party {
                    name: "Bob".to_string(),
                    bic: None,
                },
                creditor_account: Account {
                    id: "0xBobAddr".to_string(),
                },
                purpose_code: Some("SALA".to_string()),
                remittance_info: Some("January salary".to_string()),
            },
            chain_tx_hash: "0xabc123".to_string(),
            settlement_time: "2026-02-15T12:00:00".to_string(),
            instructing_agent: "MAGNUSVNX".to_string(),
            instructed_agent: "VNBKVNVX".to_string(),
        };

        let xml = build_pacs008(&settlement);
        assert!(xml.contains("E2E-001"));
        assert!(xml.contains("500.00"));
        assert!(xml.contains("Alice"));
        assert!(xml.contains("Bob"));
        assert!(xml.contains("SALA"));
        assert!(xml.contains("January salary"));
        assert!(xml.contains("0xabc123"));
        assert!(xml.contains("pacs.008.001.02"));
    }

    #[test]
    fn test_build_camt053() {
        let entries = vec![StatementEntry {
            entry_ref: "ENT-001".to_string(),
            amount: Amount {
                currency: "USD".to_string(),
                value: "1000.00".to_string(),
            },
            cd_indicator: "CRDT".to_string(),
            booking_date: "2026-02-15".to_string(),
            value_date: "2026-02-15".to_string(),
            end_to_end_id: "E2E-001".to_string(),
            remittance_info: Some("Payment received".to_string()),
        }];

        let xml = build_camt053("STMT-001", "0xAccountAddr", &entries, "2026-02-15T18:00:00");
        assert!(xml.contains("STMT-001"));
        assert!(xml.contains("0xAccountAddr"));
        assert!(xml.contains("1000.00"));
        assert!(xml.contains("CRDT"));
        assert!(xml.contains("E2E-001"));
        assert!(xml.contains("camt.053.001.02"));
    }

    #[test]
    fn test_build_camt054() {
        let notification = NotificationEntry {
            ntfctn_id: "NTFY-001".to_string(),
            account_id: "0xBobAddr".to_string(),
            entry: StatementEntry {
                entry_ref: "ENT-002".to_string(),
                amount: Amount {
                    currency: "USD".to_string(),
                    value: "250.00".to_string(),
                },
                cd_indicator: "DBIT".to_string(),
                booking_date: "2026-02-15".to_string(),
                value_date: "2026-02-15".to_string(),
                end_to_end_id: "E2E-002".to_string(),
                remittance_info: None,
            },
        };

        let xml = build_camt054(&notification, "2026-02-15T12:30:00");
        assert!(xml.contains("NTFY-001"));
        assert!(xml.contains("0xBobAddr"));
        assert!(xml.contains("250.00"));
        assert!(xml.contains("DBIT"));
        assert!(xml.contains("camt.054.001.02"));
    }

    #[test]
    fn test_pain001_parse_then_build_pacs008() {
        // Round-trip: parse pain.001 → build pacs.008 → verify fields
        let pain001 = r#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pain.001.001.03">
  <CstmrCdtTrfInitn>
    <GrpHdr>
      <MsgId>ROUND-TRIP-001</MsgId>
      <CreDtTm>2026-02-15T10:00:00</CreDtTm>
      <NbOfTxs>1</NbOfTxs>
    </GrpHdr>
    <PmtInf>
      <CdtTrfTxInf>
        <PmtId><EndToEndId>RT-E2E-001</EndToEndId></PmtId>
        <Amt><InstdAmt Ccy="USD">750.00</InstdAmt></Amt>
        <Dbtr><Nm>Sender Corp</Nm></Dbtr>
        <DbtrAcct><Id><IBAN>VN11111111111111</IBAN></Id></DbtrAcct>
        <Cdtr><Nm>Receiver Ltd</Nm></Cdtr>
        <CdtrAcct><Id><IBAN>VN22222222222222</IBAN></Id></CdtrAcct>
        <Purp><Cd>SUPP</Cd></Purp>
        <RmtInf><Ustrd>PO-2026-100</Ustrd></RmtInf>
      </CdtTrfTxInf>
    </PmtInf>
  </CstmrCdtTrfInitn>
</Document>"#;

        let instruction = parser::parse_pain001(pain001).unwrap();
        assert_eq!(instruction.transactions.len(), 1);

        let tx = instruction.transactions[0].clone();
        let settlement = SettlementData {
            instruction: tx,
            chain_tx_hash: "0xdeadbeef".to_string(),
            settlement_time: "2026-02-15T10:05:00".to_string(),
            instructing_agent: "MAGNUSVNX".to_string(),
            instructed_agent: "BANKUSVN".to_string(),
        };

        let pacs008 = build_pacs008(&settlement);
        assert!(pacs008.contains("RT-E2E-001"));
        assert!(pacs008.contains("750.00"));
        assert!(pacs008.contains("Sender Corp"));
        assert!(pacs008.contains("Receiver Ltd"));
        assert!(pacs008.contains("SUPP"));
        assert!(pacs008.contains("PO-2026-100"));
        assert!(pacs008.contains("0xdeadbeef"));
    }
}
