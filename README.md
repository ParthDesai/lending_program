# lending_program
Lending program facilitates end to end loan procedure for stable coins. It does this by offering lender to create lending pool with parameters such as expected apy, validity of loan taken from the lending pool etc.
It also offers borrower a choice to select lending pool with less apy and/or more validity and escrows collateral from borrower. 

It also utilize chainlink feed for up-to-date conversion rate between stable coin and SOL collateral.

## Instructions available

## We are making dummy changes here.

### NewLendingPool
Creates lending pool with expected apy, maximum payback time and information about which feed to use.

### NewLoan
Creates new loan with the amount. Takes collateral from borrower according to feed result and transfers the stable coin to the borrower.

### DefaultLoan
Can be called periodically by the backend. If the validity is expired, it will transfer the collateral from escrow into lending pool and loan is marked as defaulted.

### PaybackLoan
Called by borrower to payback the loan. Returns collateral to borrower and stable coins with interest are transferred to lending pool

### CloseLending
Called by Lender to close the lending pool. Lending pool can only be closed if there are no outstanding loan against it. Transfer lending pool assets to lender.
