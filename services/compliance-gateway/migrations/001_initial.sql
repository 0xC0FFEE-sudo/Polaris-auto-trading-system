-- Initial schema for compliance gateway

-- Transactions table
CREATE TABLE IF NOT EXISTS transactions (
    transaction_id VARCHAR(255) PRIMARY KEY,
    user_id VARCHAR(255) NOT NULL,
    symbol VARCHAR(50) NOT NULL,
    amount DECIMAL(20, 8) NOT NULL,
    transaction_type VARCHAR(50) NOT NULL,
    wallet_address VARCHAR(255),
    counterparty VARCHAR(255),
    timestamp TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- KYC records table
CREATE TABLE IF NOT EXISTS kyc_records (
    user_id VARCHAR(255) PRIMARY KEY,
    verification_level VARCHAR(50) NOT NULL DEFAULT 'basic',
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    documents JSONB DEFAULT '[]',
    risk_rating VARCHAR(50) NOT NULL DEFAULT 'medium',
    last_updated TIMESTAMPTZ DEFAULT NOW(),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Compliance checks table
CREATE TABLE IF NOT EXISTS compliance_checks (
    check_id VARCHAR(255) PRIMARY KEY,
    transaction_id VARCHAR(255) NOT NULL,
    rule_name VARCHAR(255) NOT NULL,
    status VARCHAR(50) NOT NULL,
    risk_score DECIMAL(5, 2) NOT NULL,
    details TEXT,
    timestamp TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    FOREIGN KEY (transaction_id) REFERENCES transactions(transaction_id)
);

-- AML alerts table
CREATE TABLE IF NOT EXISTS aml_alerts (
    alert_id VARCHAR(255) PRIMARY KEY,
    transaction_id VARCHAR(255) NOT NULL,
    alert_type VARCHAR(255) NOT NULL,
    severity VARCHAR(50) NOT NULL,
    description TEXT NOT NULL,
    risk_indicators JSONB DEFAULT '[]',
    recommended_action VARCHAR(255),
    status VARCHAR(50) DEFAULT 'open',
    assigned_to VARCHAR(255),
    resolved_at TIMESTAMPTZ,
    timestamp TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    FOREIGN KEY (transaction_id) REFERENCES transactions(transaction_id)
);

-- Risk profiles table
CREATE TABLE IF NOT EXISTS risk_profiles (
    entity_id VARCHAR(255) PRIMARY KEY,
    entity_type VARCHAR(50) NOT NULL,
    risk_score DECIMAL(5, 2) NOT NULL,
    risk_factors JSONB DEFAULT '[]',
    sanctions_check BOOLEAN DEFAULT FALSE,
    pep_check BOOLEAN DEFAULT FALSE,
    adverse_media BOOLEAN DEFAULT FALSE,
    last_updated TIMESTAMPTZ DEFAULT NOW(),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Sanctions list table
CREATE TABLE IF NOT EXISTS sanctions_list (
    id SERIAL PRIMARY KEY,
    entity_type VARCHAR(50) NOT NULL, -- 'address', 'person', 'organization'
    identifier VARCHAR(255) NOT NULL, -- wallet address, name, etc.
    list_name VARCHAR(255) NOT NULL, -- OFAC, EU, UN, etc.
    added_date TIMESTAMPTZ DEFAULT NOW(),
    is_active BOOLEAN DEFAULT TRUE
);

-- Audit log table
CREATE TABLE IF NOT EXISTS audit_log (
    id SERIAL PRIMARY KEY,
    event_type VARCHAR(255) NOT NULL,
    entity_id VARCHAR(255),
    entity_type VARCHAR(50),
    user_id VARCHAR(255),
    details JSONB,
    timestamp TIMESTAMPTZ DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_transactions_user_id ON transactions(user_id);
CREATE INDEX IF NOT EXISTS idx_transactions_timestamp ON transactions(timestamp);
CREATE INDEX IF NOT EXISTS idx_transactions_amount ON transactions(amount);
CREATE INDEX IF NOT EXISTS idx_compliance_checks_transaction_id ON compliance_checks(transaction_id);
CREATE INDEX IF NOT EXISTS idx_compliance_checks_timestamp ON compliance_checks(timestamp);
CREATE INDEX IF NOT EXISTS idx_aml_alerts_transaction_id ON aml_alerts(transaction_id);
CREATE INDEX IF NOT EXISTS idx_aml_alerts_severity ON aml_alerts(severity);
CREATE INDEX IF NOT EXISTS idx_aml_alerts_status ON aml_alerts(status);
CREATE INDEX IF NOT EXISTS idx_risk_profiles_entity_type ON risk_profiles(entity_type);
CREATE INDEX IF NOT EXISTS idx_sanctions_list_identifier ON sanctions_list(identifier);
CREATE INDEX IF NOT EXISTS idx_audit_log_timestamp ON audit_log(timestamp);
CREATE INDEX IF NOT EXISTS idx_audit_log_entity_id ON audit_log(entity_id);