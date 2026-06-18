[CDN]

- Service Domain
    - sklb-test.dn.nexoncdn.co.kr
- Test File URL
    - http://sklb-test.dn.nexoncdn.co.kr/sklb-test.dn.nexoncdn.co.kr.txt

[Origin Storage(S3)]

- Primary Bucket Name
    - sklb-test-test-kr-online-nexoncdn
- Region
    - 한국(Seoul)
- Full Path EndPoint(CDN Origin)
    - sklb-test-test-kr-online-nexoncdn.s3.ap-northeast-2.amazonaws.com/contents/
    - S3의 /contents/ 경로가 CDN의 루트 / 경로로 매핑\
    - Test File URL
        - sklb-test-test-kr-online-nexoncdn.s3.ap-northeast-2.amazonaws.com/contents/sklb-test.dn.nexoncdn.co.kr.txt
        - referer 헤더 설정으로 S3로 직접다운로드 불가

[AKAMAI]
- HOST : EdgeGrid 호스트명
- CLIENT_TOKEN : EdgeGrid Client Token
- CLIENT_SECRET : EdgeGrid Client Secret
- ACCESS_TOKEN : EdgeGrid Access Token
- CP_CODE : 퍼지대상 CP CODE
[CLOUDFRONT]
- AWS_ACCESS_KEY_ID : S3 업로드 겸용 IAM(CloudFront Invalidation 권한 포함)
- AWS_SECRET_ACCESS_KEY 
- DISTRIBUTION_ID : 퍼지 대상 Distribution ID

Akamai	sklb-test.dn.nexoncdn.co.kr.edgesuite.net
CloudFront	d2f1611qj8oisv.cloudfront.net